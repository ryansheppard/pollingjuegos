//! Pure helpers kept out of the view code so they can be tested.

use std::cmp::Reverse;
use std::collections::HashMap;

use api_types::{Ballot, Quarterback, Rankings, Week};
use jiff::Timestamp;
use jiff::tz::TimeZone;

/// The server only flips `is_closed` when the list is fetched, so a deadline
/// can pass while the app is open.
pub fn is_closed(week: &Week, now: Timestamp) -> bool {
    week.is_closed || week.closes_at.is_some_and(|closes| now >= closes)
}

/// Defaults to the oldest week still taking votes, or the most recent one if
/// every week is closed. Returns an index into `weeks`.
pub fn initial_week(weeks: &[Week], now: Timestamp) -> Option<usize> {
    let chronological = |(_, w): &(usize, &Week)| (w.season, w.week);
    weeks
        .iter()
        .enumerate()
        .filter(|(_, w)| !is_closed(w, now))
        .min_by_key(chronological)
        .or_else(|| weeks.iter().enumerate().max_by_key(chronological))
        .map(|(index, _)| index)
}

pub fn week_label(week: &Week) -> String {
    format!("{} · Week {}", week.season, week.week)
}

pub fn closes_label(week: &Week) -> String {
    match week.closes_at {
        Some(closes) => closes
            .to_zoned(TimeZone::system())
            .strftime("%a, %b %-d, %-I:%M %p")
            .to_string(),
        None => "No deadline".to_owned(),
    }
}

pub fn time_left(week: &Week, now: Timestamp) -> String {
    if is_closed(week, now) {
        return "Closed".to_owned();
    }
    let Some(closes) = week.closes_at else {
        return "Open".to_owned();
    };
    let minutes = closes.duration_since(now).as_secs().max(0) / 60;
    let (d, h, m) = (minutes / 1440, minutes % 1440 / 60, minutes % 60);
    if d > 0 {
        format!("{d}d {h}h {m:02}m")
    } else if h > 0 {
        format!("{h}h {m:02}m")
    } else {
        format!("{m}m")
    }
}

pub fn qb_label(qb: &Quarterback) -> String {
    match &qb.team {
        Some(team) => format!("{} ({})", qb.name, team.abbreviation),
        None => qb.name.clone(),
    }
}

pub struct Disagreement<'a> {
    pub quarterback: &'a Quarterback,
    pub mine: i64,
    /// `None` when the pick isn't in the crowd's top 10.
    pub crowd: Option<i64>,
}

/// The saved picks that differ most from the crowd's ranking, biggest gap first.
pub fn disagreements<'a>(saved: &'a Ballot, rankings: &Rankings) -> Vec<Disagreement<'a>> {
    let crowd: HashMap<i64, i64> = rankings
        .rankings
        .iter()
        .map(|row| (row.quarterback.id, row.rank))
        .collect();
    let mut rows: Vec<_> = saved
        .entries
        .iter()
        .map(|entry| Disagreement {
            quarterback: &entry.quarterback,
            mine: entry.rank,
            crowd: crowd.get(&entry.quarterback.id).copied(),
        })
        .filter(|row| row.crowd != Some(row.mine))
        .collect();
    // Picks outside the top 10 count as if ranked just below it.
    rows.sort_by_key(|row| Reverse((row.mine - row.crowd.unwrap_or(11)).abs()));
    rows.truncate(3);
    rows
}

/// Plain-text stand-in for the web app's shareable results image.
pub fn results_text(week: &Week, rankings: &Rankings) -> String {
    let mut out = format!(
        "PollingJuegos · {} top 10\n{} ballots · 15 points for #1 through 1 for #15\n",
        week_label(week),
        rankings.ballots
    );
    for row in &rankings.rankings {
        out.push_str(&format!(
            "\n{:>2}. {} · {} pts",
            row.rank,
            qb_label(&row.quarterback),
            row.points
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use api_types::{BallotEntry, RankingEntry};

    use super::*;

    fn week(season: i64, week: i64, closes_at: Option<&str>, is_closed: bool) -> Week {
        Week {
            id: 0,
            season,
            week,
            closes_at: closes_at.map(|t| t.parse().unwrap()),
            is_closed,
        }
    }

    const NOW: &str = "2026-11-07T12:00:00Z";

    #[test]
    fn initial_week_prefers_oldest_open_then_newest() {
        let now = NOW.parse().unwrap();
        let weeks = [
            week(2026, 10, None, false),
            week(2026, 9, Some("2026-11-08T00:00:00Z"), false),
            // Not yet marked closed by the server, but its deadline has passed.
            week(2026, 8, Some("2026-11-01T00:00:00Z"), false),
        ];
        assert_eq!(initial_week(&weeks, now), Some(1));

        let all_closed = [week(2025, 18, None, true), week(2026, 1, None, true)];
        assert_eq!(initial_week(&all_closed, now), Some(1));
        assert_eq!(initial_week(&[], now), None);
    }

    #[test]
    fn time_left_formats() {
        let now = NOW.parse().unwrap();
        let left = |closes| time_left(&week(2026, 1, Some(closes), false), now);
        assert_eq!(left("2026-11-09T15:05:30Z"), "2d 3h 05m");
        assert_eq!(left("2026-11-07T13:09:00Z"), "1h 09m");
        assert_eq!(left("2026-11-07T12:00:59Z"), "0m");
        assert_eq!(left("2026-11-07T11:00:00Z"), "Closed");
        assert_eq!(time_left(&week(2026, 1, None, false), now), "Open");
    }

    fn qb(id: i64) -> Quarterback {
        Quarterback {
            id,
            name: format!("QB {id}"),
            team: None,
        }
    }

    #[test]
    fn disagreements_rank_biggest_gaps_first() {
        let saved = Ballot {
            season: 2026,
            week: 1,
            entries: (1..=15)
                .map(|rank| BallotEntry {
                    rank,
                    quarterback: qb(rank),
                })
                .collect(),
        };
        // The crowd agrees on QB 1, swaps QBs 2 and 3, and puts QB 15 at #4.
        let crowd = [(1, 1), (3, 2), (2, 3), (15, 4)];
        let rankings = Rankings {
            season: 2026,
            week: 1,
            ballots: 3,
            rankings: crowd
                .iter()
                .map(|&(id, rank)| RankingEntry {
                    rank,
                    points: 0,
                    votes: 0,
                    previous_rank: None,
                    quarterback: qb(id),
                })
                .collect(),
        };
        let got: Vec<_> = disagreements(&saved, &rankings)
            .iter()
            .map(|row| (row.quarterback.id, row.mine, row.crowd))
            .collect();
        // QB 15 is 11 off; unranked QBs 4 and 5 are 7 and 6 from the
        // "just outside" #11.
        assert_eq!(got, [(15, 15, Some(4)), (4, 4, None), (5, 5, None)]);
    }
}
