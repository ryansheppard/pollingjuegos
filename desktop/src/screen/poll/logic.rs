//! Pure helpers kept out of the view code so they can be tested.

use api_types::{Quarterback, Week};
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

#[cfg(test)]
mod tests {
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
}
