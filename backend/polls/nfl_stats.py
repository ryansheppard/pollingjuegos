"""Current-season QB stats from nflverse; read-through cached, never stored in the DB."""

import logging
import re
import threading
import time

import nflreadpy
import polars as pl
from nflreadpy.downloader import get_downloader

logger = logging.getLogger(__name__)

# nflreadpy's own cache decides when files are re-downloaded; this only spares
# re-parsing them on every request.
CACHE_SECONDS = 15 * 60
# Avoid hammering GitHub (and blocking requests on its timeout) during an outage.
FAILURE_CACHE_SECONDS = 5 * 60
# Keeps spot-duty backups off per-play leaderboards.
MIN_RANKED_DROPBACKS = 100
_RANK_KEYS = {"qbr": "qbr_rank", "epa_per_dropback": "epa_rank", "cpoe": "cpoe_rank"}

_lock = threading.Lock()
_cached: tuple[float, dict[str, dict]] | None = None

_SUFFIX = re.compile(r"\s+(jr|sr|ii|iii|iv|v)$")


def normalize_name(name: str) -> str:
    """Seed names carry suffixes and punctuation that nflverse/ESPN spell differently."""
    name = re.sub(r"[.'’]", "", name.lower()).replace("-", " ")
    return _SUFFIX.sub("", " ".join(name.split()))


def _load(season: int) -> dict[str, dict]:
    stats = (
        nflreadpy.load_player_stats(season, summary_level="reg+post")
        .filter(pl.col("position") == "QB")
        .with_columns(dropbacks=pl.col("attempts") + pl.col("sacks_suffered"))
    )
    # ESPN QBR isn't exposed by nflreadpy, but nflverse mirrors it; going through
    # the downloader shares nflreadpy's cache.
    qbr = (
        get_downloader()
        .download("nflverse-data", "espn_data/qbr_season_level")
        .filter(pl.col("season") == season)
        .with_columns(key=pl.col("name_display").map_elements(normalize_name))
    )
    # ESPN only publishes regular season and playoffs separately; a play-weighted
    # mean approximates the combined figure but isn't an official QBR.
    qbr_by_name = dict(
        qbr.group_by("key")
        .agg(
            (pl.col("qbr_total") * pl.col("qb_plays")).sum() / pl.col("qb_plays").sum()
        )
        .iter_rows()
    )

    result = {}
    # Ascending dropbacks so a same-named backup never overwrites the starter.
    for row in stats.sort("dropbacks").iter_rows(named=True):
        key = normalize_name(row["player_display_name"])
        dropbacks = row["dropbacks"] or 0
        result[key] = {
            "season": season,
            "games": row["games"],
            "dropbacks": dropbacks,
            "epa": row["passing_epa"],
            "epa_per_dropback": (
                row["passing_epa"] / dropbacks
                if dropbacks and row["passing_epa"] is not None
                else None
            ),
            "cpoe": row["passing_cpoe"],
            "qbr": qbr_by_name.get(key),
        }
    # After deduping by name, so a shadowed backup can't take a rank slot.
    add_league_ranks(list(result.values()))
    return result


def add_league_ranks(lines: list[dict]) -> None:
    """Rank qualified QBs per stat, higher is better; ties share the best rank."""
    qualified = [line for line in lines if line["dropbacks"] >= MIN_RANKED_DROPBACKS]
    for line in lines:
        line["qualified"] = line["dropbacks"] >= MIN_RANKED_DROPBACKS
        line["qualified_count"] = len(qualified)
        for rank_key in _RANK_KEYS.values():
            line[rank_key] = None
    for stat, rank_key in _RANK_KEYS.items():
        values = [line[stat] for line in qualified if line[stat] is not None]
        for line in qualified:
            if line[stat] is not None:
                line[rank_key] = 1 + sum(value > line[stat] for value in values)


def qb_stats_by_name() -> dict[str, dict]:
    """Season-to-date regular-season stats keyed by normalize_name()."""
    global _cached
    with _lock:
        if _cached is not None and time.monotonic() < _cached[0]:
            return _cached[1]
        try:
            stats = _load(nflreadpy.get_current_season())
            ttl = CACHE_SECONDS
        except Exception:
            logger.exception("Failed to load nflverse QB stats")
            stats = _cached[1] if _cached is not None else {}
            ttl = FAILURE_CACHE_SECONDS
        _cached = (time.monotonic() + ttl, stats)
        return stats


def stats_for(name: str) -> dict | None:
    return qb_stats_by_name().get(normalize_name(name))
