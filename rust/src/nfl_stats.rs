//! Current-season QB stats from nflverse; refreshed in the background and
//! held in memory, never stored in the DB.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::{Context, anyhow, bail};
use api_types::QuarterbackStats;
use bytes::Bytes;
use jiff::{Timestamp, civil, tz::TimeZone};
use ntex::http::header;
use ntex::time::{Millis, sleep};
use parquet::file::reader::{FileReader, SerializedFileReader};
use parquet::record::{Field, Row};
use parquet::schema::types::Type;

const RELEASES: &str = "https://github.com/nflverse/nflverse-data/releases/download";
const REFRESH: Duration = Duration::from_secs(60 * 60);
// Retry sooner after a failure, but not so soon that a GitHub outage gets hammered.
const RETRY: Duration = Duration::from_secs(5 * 60);
// The season-to-date player file grows weekly and is already near ntex's 256 KiB default.
const MAX_DOWNLOAD: usize = 16 * 1024 * 1024;
// Keeps spot-duty backups off per-play leaderboards; `QuarterbackStats` ranks
// only cover QBs with at least this many.
const MIN_RANKED_DROPBACKS: i64 = 100;

pub type ByName = HashMap<String, QuarterbackStats>;

/// Shared by every worker; readers clone the inner `Arc` so a refresh never
/// blocks a request for longer than a pointer swap.
#[derive(Clone, Default)]
pub struct NflStats(Arc<RwLock<Arc<ByName>>>);

impl NflStats {
    pub fn snapshot(&self) -> Arc<ByName> {
        self.0.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn stats_for(stats: &ByName, name: &str) -> Option<QuarterbackStats> {
        stats.get(&normalize_name(name)).cloned()
    }

    pub(crate) fn replace(&self, stats: ByName) {
        *self.0.write().unwrap_or_else(|e| e.into_inner()) = Arc::new(stats);
    }

    /// Loads immediately, then keeps refreshing; on failure the last good
    /// snapshot stays in place.
    pub async fn refresh_forever(self, http: ntex::client::Client) {
        loop {
            let delay = match load(&http, current_season(today_utc())).await {
                Ok(stats) => {
                    self.replace(stats);
                    REFRESH
                }
                Err(e) => {
                    eprintln!("failed to load nflverse QB stats: {e:#}");
                    RETRY
                }
            };
            sleep(Millis::from(delay)).await;
        }
    }
}

fn today_utc() -> civil::Date {
    Timestamp::now().to_zoned(TimeZone::UTC).date()
}

/// Mirrors nflreadpy's `get_current_season`: the season rolls over on the
/// Thursday after Labor Day.
pub fn current_season(today: civil::Date) -> i16 {
    let year = today.year();
    let sept1 = civil::date(year, 9, 1);
    let first_monday = 1 + (7 - sept1.weekday().to_monday_zero_offset()) % 7;
    if today >= civil::date(year, 9, first_monday + 3) {
        year
    } else {
        year - 1
    }
}

/// Seed names carry suffixes and punctuation that nflverse/ESPN spell differently.
pub fn normalize_name(name: &str) -> String {
    let cleaned: String = name
        .to_lowercase()
        .chars()
        .filter(|c| !matches!(c, '.' | '\'' | '’'))
        .map(|c| if c == '-' { ' ' } else { c })
        .collect();
    let mut words: Vec<&str> = cleaned.split_whitespace().collect();
    if words.len() > 1
        && matches!(
            words.last(),
            Some(&("jr" | "sr" | "ii" | "iii" | "iv" | "v"))
        )
    {
        words.pop();
    }
    words.join(" ")
}

async fn load(http: &ntex::client::Client, season: i16) -> anyhow::Result<ByName> {
    let players = fetch(
        http,
        &format!("{RELEASES}/stats_player/stats_player_regpost_{season}.parquet"),
    )
    .await?;
    let qbr = fetch(
        http,
        &format!("{RELEASES}/espn_data/qbr_season_level.parquet"),
    )
    .await?;
    Ok(build(
        season.into(),
        parse_players(players).context("parsing player stats")?,
        parse_qbr(qbr, season.into()).context("parsing QBR")?,
    ))
}

async fn fetch(http: &ntex::client::Client, url: &str) -> anyhow::Result<Bytes> {
    let mut url = url.to_owned();
    // Release downloads redirect to a signed asset URL; ntex doesn't follow redirects.
    for _ in 0..3 {
        let res = http
            .get(&url)
            .timeout(Millis(30_000))
            .send()
            .await
            .map_err(|e| anyhow!("GET {url}: {e}"))?;
        if res.status().is_redirection() {
            url = res
                .headers()
                .get(header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .with_context(|| format!("GET {url}: redirect without Location"))?
                .to_owned();
            continue;
        }
        if !res.status().is_success() {
            bail!("GET {url} returned {}", res.status());
        }
        let body = res
            .body()
            .limit(MAX_DOWNLOAD)
            .timeout(Millis(30_000))
            .await
            .map_err(|e| anyhow!("reading {url}: {e}"))?;
        return Ok(Bytes::copy_from_slice(&body));
    }
    bail!("too many redirects fetching {url}")
}

#[derive(Debug, Default)]
struct PlayerRow {
    name: String,
    position: Option<String>,
    games: Option<i64>,
    attempts: Option<i64>,
    sacks: Option<i64>,
    epa: Option<f64>,
    cpoe: Option<f64>,
}

#[derive(Debug, Default)]
struct QbrRow {
    name: String,
    season: Option<i64>,
    qbr: Option<f64>,
    plays: Option<f64>,
}

/// Visits each row's projected columns; both files have ~150 columns we don't need.
fn for_each_row(data: Bytes, columns: &[&str], mut visit: impl FnMut(&Row)) -> anyhow::Result<()> {
    let reader = SerializedFileReader::new(data)?;
    let schema = reader.metadata().file_metadata().schema();
    let fields = schema
        .get_fields()
        .iter()
        .filter(|f| columns.contains(&f.name()))
        .cloned()
        .collect::<Vec<_>>();
    if fields.len() != columns.len() {
        bail!("expected columns {columns:?}");
    }
    let projection = Type::group_type_builder(schema.name())
        .with_fields(fields)
        .build()?;
    for row in reader.get_row_iter(Some(projection))? {
        visit(&row?);
    }
    Ok(())
}

fn as_i64(f: &Field) -> Option<i64> {
    match *f {
        Field::Short(v) => Some(v.into()),
        Field::Int(v) => Some(v.into()),
        Field::Long(v) => Some(v),
        _ => None,
    }
}

fn as_f64(f: &Field) -> Option<f64> {
    match *f {
        Field::Float(v) => Some(v.into()),
        Field::Double(v) => Some(v),
        _ => as_i64(f).map(|v| v as f64),
    }
}

fn as_string(f: &Field) -> Option<String> {
    match f {
        Field::Str(s) => Some(s.clone()),
        _ => None,
    }
}

fn parse_players(data: Bytes) -> anyhow::Result<Vec<PlayerRow>> {
    let mut rows = Vec::new();
    let columns = [
        "player_display_name",
        "position",
        "games",
        "attempts",
        "sacks_suffered",
        "passing_epa",
        "passing_cpoe",
    ];
    for_each_row(data, &columns, |row| {
        let mut p = PlayerRow::default();
        for (name, f) in row.get_column_iter() {
            match name.as_str() {
                "player_display_name" => p.name = as_string(f).unwrap_or_default(),
                "position" => p.position = as_string(f),
                "games" => p.games = as_i64(f),
                "attempts" => p.attempts = as_i64(f),
                "sacks_suffered" => p.sacks = as_i64(f),
                "passing_epa" => p.epa = as_f64(f),
                "passing_cpoe" => p.cpoe = as_f64(f),
                _ => {}
            }
        }
        rows.push(p);
    })?;
    Ok(rows)
}

/// ESPN QBR isn't in the player stats file, but nflverse mirrors it.
fn parse_qbr(data: Bytes, season: i64) -> anyhow::Result<Vec<QbrRow>> {
    let mut rows = Vec::new();
    let columns = ["season", "name_display", "qbr_total", "qb_plays"];
    for_each_row(data, &columns, |row| {
        let mut q = QbrRow::default();
        for (name, f) in row.get_column_iter() {
            match name.as_str() {
                "season" => q.season = as_i64(f),
                "name_display" => q.name = as_string(f).unwrap_or_default(),
                "qbr_total" => q.qbr = as_f64(f),
                "qb_plays" => q.plays = as_f64(f),
                _ => {}
            }
        }
        if q.season == Some(season) {
            rows.push(q);
        }
    })?;
    Ok(rows)
}

fn build(season: i64, players: Vec<PlayerRow>, qbr: Vec<QbrRow>) -> ByName {
    // ESPN only publishes regular season and playoffs separately; a play-weighted
    // mean approximates the combined figure but isn't an official QBR.
    let mut qbr_sums: HashMap<String, (f64, f64)> = HashMap::new();
    for row in qbr {
        if let (Some(q), Some(p)) = (row.qbr, row.plays) {
            let sum = qbr_sums.entry(normalize_name(&row.name)).or_default();
            sum.0 += q * p;
            sum.1 += p;
        }
    }

    let mut qbs: Vec<(i64, PlayerRow)> = players
        .into_iter()
        .filter(|p| p.position.as_deref() == Some("QB"))
        .map(|p| {
            let dropbacks = p.attempts.zip(p.sacks).map_or(0, |(a, s)| a + s);
            (dropbacks, p)
        })
        .collect();
    // Ascending dropbacks so a same-named backup never overwrites the starter.
    qbs.sort_by_key(|(dropbacks, _)| *dropbacks);

    let mut result = ByName::new();
    for (dropbacks, p) in qbs {
        let key = normalize_name(&p.name);
        let qbr = qbr_sums
            .get(&key)
            .and_then(|&(weighted, plays)| (plays > 0.0).then(|| weighted / plays));
        result.insert(
            key,
            QuarterbackStats {
                season,
                games: p.games.unwrap_or(0),
                dropbacks,
                epa: p.epa,
                epa_per_dropback: p
                    .epa
                    .filter(|_| dropbacks != 0)
                    .map(|e| e / dropbacks as f64),
                cpoe: p.cpoe,
                qbr,
                qualified: false,
                qualified_count: 0,
                qbr_rank: None,
                epa_rank: None,
                cpoe_rank: None,
            },
        );
    }
    // After deduping by name, so a shadowed backup can't take a rank slot.
    add_league_ranks(result.values_mut().collect());
    result
}

/// Rank qualified QBs per stat, higher is better; ties share the best rank.
fn add_league_ranks(mut lines: Vec<&mut QuarterbackStats>) {
    let qualified_count = lines
        .iter()
        .filter(|l| l.dropbacks >= MIN_RANKED_DROPBACKS)
        .count() as i64;
    for line in &mut lines {
        line.qualified = line.dropbacks >= MIN_RANKED_DROPBACKS;
        line.qualified_count = qualified_count;
    }
    rank_by(&mut lines, |l| l.qbr, |l| &mut l.qbr_rank);
    rank_by(&mut lines, |l| l.epa_per_dropback, |l| &mut l.epa_rank);
    rank_by(&mut lines, |l| l.cpoe, |l| &mut l.cpoe_rank);
}

fn rank_by(
    lines: &mut [&mut QuarterbackStats],
    get: impl Fn(&QuarterbackStats) -> Option<f64>,
    rank: impl Fn(&mut QuarterbackStats) -> &mut Option<i64>,
) {
    let values: Vec<f64> = lines
        .iter()
        .filter(|l| l.qualified)
        .filter_map(|l| get(l))
        .collect();
    for line in lines.iter_mut().filter(|l| l.qualified) {
        *rank(line) = get(line).map(|v| 1 + values.iter().filter(|&&o| o > v).count() as i64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_seed_and_nflverse_spellings_alike() {
        assert_eq!(normalize_name("Michael Penix Jr."), "michael penix");
        assert_eq!(normalize_name("Amon-Ra  St. Brown"), "amon ra st brown");
        assert_eq!(normalize_name("D’Andre O'Neal III"), "dandre oneal");
        // A lone suffix-looking word is a name, not a suffix.
        assert_eq!(normalize_name("V"), "v");
    }

    #[test]
    fn season_rolls_over_thursday_after_labor_day() {
        // Labor Day 2026 is Sept 7.
        assert_eq!(current_season(civil::date(2026, 9, 9)), 2025);
        assert_eq!(current_season(civil::date(2026, 9, 10)), 2026);
        assert_eq!(current_season(civil::date(2027, 2, 1)), 2026);
        // Labor Day 2025 was Sept 1, the first day of the month.
        assert_eq!(current_season(civil::date(2025, 9, 4)), 2025);
    }

    fn qb(name: &str, attempts: i64, epa: f64, cpoe: Option<f64>) -> PlayerRow {
        PlayerRow {
            name: name.into(),
            position: Some("QB".into()),
            games: Some(5),
            attempts: Some(attempts),
            sacks: Some(0),
            epa: Some(epa),
            cpoe,
        }
    }

    #[test]
    fn builds_ranks_and_dedupes_by_name() {
        let players = vec![
            qb("Josh Allen", 200, 40.0, Some(3.0)),
            // Same normalized name, fewer dropbacks: must not shadow the starter.
            qb("Josh Allen", 10, 50.0, Some(9.0)),
            qb("Joe Burrow", 200, 40.0, None),
            qb("Backup Guy", 50, 100.0, Some(10.0)),
            PlayerRow {
                position: Some("WR".into()),
                ..qb("Not A QB", 500, 1.0, None)
            },
        ];
        let qbr = vec![
            QbrRow {
                name: "Josh Allen".into(),
                season: Some(2026),
                qbr: Some(80.0),
                plays: Some(300.0),
            },
            QbrRow {
                name: "Josh Allen".into(),
                season: Some(2026),
                qbr: Some(40.0),
                plays: Some(100.0),
            },
        ];
        let stats = build(2026, players, qbr);

        assert_eq!(stats.len(), 3);
        let allen = &stats["josh allen"];
        assert_eq!(allen.dropbacks, 200);
        assert_eq!(allen.epa_per_dropback, Some(0.2));
        assert_eq!(allen.qbr, Some(70.0));
        assert_eq!(
            (allen.qbr_rank, allen.epa_rank, allen.cpoe_rank),
            (Some(1), Some(1), Some(1))
        );
        assert_eq!(allen.qualified_count, 2);

        let burrow = &stats["joe burrow"];
        assert_eq!(
            (burrow.epa_rank, burrow.cpoe_rank, burrow.qbr),
            (Some(1), None, None)
        );

        let backup = &stats["backup guy"];
        assert!(!backup.qualified);
        assert_eq!(backup.epa_rank, None);
    }
}
