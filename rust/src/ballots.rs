use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};

use crate::error::{AppError, Result};
use crate::models::{
    BallotEntry, BallotOut, QuarterbackWithTeam, RankingEntry, RankingsOut, TeamOut,
};

pub const BALLOT_SIZE: i64 = 15;
const RANKINGS_SIZE: i64 = 10;

/// Selects `q.id, q.name` plus the team columns `qb_from_row` expects; callers
/// must alias `quarterbacks` as `q` and LEFT JOIN `teams` as `t`.
const QB_COLUMNS: &str = "q.id AS qb_id, q.name AS qb_name,
    t.id AS team_id, t.name AS team_name, t.abbreviation AS team_abbreviation";

fn qb_from_row(row: &Row) -> rusqlite::Result<QuarterbackWithTeam> {
    let team_id: Option<i64> = row.get("team_id")?;
    Ok(QuarterbackWithTeam {
        id: row.get("qb_id")?,
        name: row.get("qb_name")?,
        team: match team_id {
            Some(id) => Some(TeamOut {
                id,
                name: row.get("team_name")?,
                abbreviation: row.get("team_abbreviation")?,
            }),
            None => None,
        },
    })
}

fn week_id(conn: &Connection, season: i64, week: i64) -> Result<i64> {
    conn.prepare_cached("SELECT id FROM poll_weeks WHERE season = ?1 AND week = ?2")?
        .query_row([season, week], |row| row.get(0))
        .optional()?
        .ok_or(AppError::NotFound)
}

pub fn get_ballot(conn: &Connection, season: i64, week: i64, voter_id: i64) -> Result<BallotOut> {
    let poll_week_id = week_id(conn, season, week)?;
    let entries = conn
        .prepare_cached(&format!(
            "SELECT v.rank, {QB_COLUMNS}
             FROM ballots b
             JOIN votes v ON v.ballot_id = b.id
             JOIN quarterbacks q ON q.id = v.quarterback_id
             LEFT JOIN teams t ON t.id = q.team_id
             WHERE b.poll_week_id = ?1 AND b.voter_id = ?2
             ORDER BY v.rank"
        ))?
        .query_map([poll_week_id, voter_id], |row| {
            Ok(BallotEntry {
                rank: row.get("rank")?,
                quarterback: qb_from_row(row)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(BallotOut {
        season,
        week,
        entries,
    })
}

/// Creates or replaces the voter's whole ballot; ranks come from list order.
pub fn submit_ballot(
    conn: &Connection,
    season: i64,
    week: i64,
    voter_id: i64,
    quarterback_ids: &[i64],
) -> Result<BallotOut> {
    // IMMEDIATE takes the write lock up front: a deferred transaction that reads
    // then writes can hit SQLITE_BUSY without busy_timeout ever retrying it.
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let poll_week_id = week_id(&tx, season, week)?;

    let mut distinct = quarterback_ids.to_vec();
    distinct.sort_unstable();
    distinct.dedup();
    if quarterback_ids.len() as i64 != BALLOT_SIZE || distinct.len() as i64 != BALLOT_SIZE {
        return Err(AppError::BadRequest(
            "A ballot must contain exactly 15 distinct quarterbacks",
        ));
    }
    let existing: i64 = tx
        .prepare_cached(
            "SELECT count(*) FROM quarterbacks WHERE id IN (SELECT value FROM json_each(?1))",
        )?
        .query_row(
            [serde_json::to_string(&distinct).expect("i64s encode")],
            |row| row.get(0),
        )?;
    if existing != BALLOT_SIZE {
        return Err(AppError::BadRequest(
            "One or more quarterbacks do not exist",
        ));
    }

    let closed: bool = tx
        .prepare_cached(
            "SELECT closes_at IS NOT NULL AND closes_at <= strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
             FROM poll_weeks WHERE id = ?1",
        )?
        .query_row([poll_week_id], |row| row.get(0))?;
    if closed {
        return Err(AppError::BadRequest("Voting for this week has closed"));
    }

    // The no-op DO UPDATE makes RETURNING yield the existing row's id on conflict.
    let ballot_id: i64 = tx
        .prepare_cached(
            "INSERT INTO ballots (poll_week_id, voter_id) VALUES (?1, ?2)
             ON CONFLICT (poll_week_id, voter_id) DO UPDATE SET voter_id = excluded.voter_id
             RETURNING id",
        )?
        .query_row([poll_week_id, voter_id], |row| row.get(0))?;
    tx.execute("DELETE FROM votes WHERE ballot_id = ?1", [ballot_id])?;
    {
        let mut insert = tx.prepare_cached(
            "INSERT INTO votes (ballot_id, quarterback_id, rank) VALUES (?1, ?2, ?3)",
        )?;
        for (rank, qb_id) in (1i64..).zip(quarterback_ids) {
            insert.execute(params![ballot_id, qb_id, rank])?;
        }
    }
    tx.commit()?;

    get_ballot(conn, season, week, voter_id)
}

struct Score {
    quarterback: QuarterbackWithTeam,
    points: i64,
    votes: i64,
}

/// AP-style: #1 earns `BALLOT_SIZE` points down to 1 point for last.
fn top_scores(conn: &Connection, poll_week_id: i64) -> Result<Vec<Score>> {
    Ok(conn
        .prepare_cached(&format!(
            "SELECT {QB_COLUMNS}, sum(?2 + 1 - v.rank) AS points, count(*) AS votes
             FROM votes v
             JOIN ballots b ON b.id = v.ballot_id
             JOIN quarterbacks q ON q.id = v.quarterback_id
             LEFT JOIN teams t ON t.id = q.team_id
             WHERE b.poll_week_id = ?1
             GROUP BY q.id
             ORDER BY points DESC, votes DESC, q.name, q.id
             LIMIT ?3"
        ))?
        .query_map(params![poll_week_id, BALLOT_SIZE, RANKINGS_SIZE], |row| {
            Ok(Score {
                quarterback: qb_from_row(row)?,
                points: row.get("points")?,
                votes: row.get("votes")?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Movement compares with the closest earlier week that exists, not strictly week - 1.
pub fn rankings(conn: &Connection, season: i64, week: i64) -> Result<RankingsOut> {
    let poll_week_id = week_id(conn, season, week)?;
    let scores = top_scores(conn, poll_week_id)?;

    let previous_week: Option<i64> = conn
        .prepare_cached(
            "SELECT id FROM poll_weeks
             WHERE season < ?1 OR (season = ?1 AND week < ?2)
             ORDER BY season DESC, week DESC
             LIMIT 1",
        )?
        .query_row([season, week], |row| row.get(0))
        .optional()?;
    let previous = match previous_week {
        Some(id) => top_scores(conn, id)?,
        None => Vec::new(),
    };
    let previous_rank = |qb_id: i64| {
        (1i64..)
            .zip(&previous)
            .find(|(_, s)| s.quarterback.id == qb_id)
            .map(|(rank, _)| rank)
    };

    let ballots: i64 = conn
        .prepare_cached("SELECT count(*) FROM ballots WHERE poll_week_id = ?1")?
        .query_row([poll_week_id], |row| row.get(0))?;

    Ok(RankingsOut {
        season,
        week,
        ballots,
        rankings: (1i64..)
            .zip(scores)
            .map(|(rank, s)| RankingEntry {
                rank,
                points: s.points,
                votes: s.votes,
                previous_rank: previous_rank(s.quarterback.id),
                quarterback: s.quarterback,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn setup() -> Connection {
        let conn = db::test_conn();
        conn.execute_batch(
            "INSERT INTO users (discord_id, username) VALUES ('1', 'a'), ('2', 'b');
             INSERT INTO poll_weeks (season, week) VALUES (2025, 18), (2026, 1), (2026, 3);",
        )
        .unwrap();
        conn
    }

    fn qb_ids(conn: &Connection, n: usize) -> Vec<i64> {
        conn.prepare("SELECT id FROM quarterbacks ORDER BY id LIMIT ?1")
            .unwrap()
            .query_map([n as i64], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    fn detail(r: Result<BallotOut>) -> &'static str {
        match r {
            Err(AppError::BadRequest(d)) => d,
            other => panic!(
                "expected BadRequest, got {:?}",
                other.map(|b| b.entries.len())
            ),
        }
    }

    #[test]
    fn submit_then_replace_ballot() {
        let conn = setup();
        let ids = qb_ids(&conn, 16);
        let first = submit_ballot(&conn, 2026, 1, 1, &ids[..15]).unwrap();
        assert_eq!(first.entries.len(), 15);
        assert_eq!(first.entries[0].quarterback.id, ids[0]);
        assert!(first.entries[0].quarterback.team.is_some());

        let mut reordered: Vec<i64> = ids[1..].to_vec();
        reordered.reverse();
        submit_ballot(&conn, 2026, 1, 1, &reordered).unwrap();
        let got = get_ballot(&conn, 2026, 1, 1).unwrap();
        let got_ids: Vec<i64> = got.entries.iter().map(|e| e.quarterback.id).collect();
        assert_eq!(got_ids, reordered);
        assert_eq!(got.entries.last().unwrap().rank, 15);

        let ballots: i64 = conn
            .query_row("SELECT count(*) FROM ballots", [], |r| r.get(0))
            .unwrap();
        assert_eq!(ballots, 1);
    }

    #[test]
    fn empty_ballot_for_non_voter() {
        let conn = setup();
        assert!(get_ballot(&conn, 2026, 1, 2).unwrap().entries.is_empty());
    }

    #[test]
    fn unknown_week_is_not_found() {
        let conn = setup();
        assert!(matches!(
            get_ballot(&conn, 2026, 2, 1),
            Err(AppError::NotFound)
        ));
        assert!(matches!(rankings(&conn, 2026, 2), Err(AppError::NotFound)));
    }

    #[test]
    fn rejects_invalid_ballots() {
        let conn = setup();
        let ids = qb_ids(&conn, 15);

        assert_eq!(
            detail(submit_ballot(&conn, 2026, 1, 1, &ids[..14])),
            "A ballot must contain exactly 15 distinct quarterbacks"
        );
        let mut dup = ids.clone();
        dup[14] = dup[0];
        assert_eq!(
            detail(submit_ballot(&conn, 2026, 1, 1, &dup)),
            "A ballot must contain exactly 15 distinct quarterbacks"
        );
        let mut missing = ids.clone();
        missing[14] = 999_999;
        assert_eq!(
            detail(submit_ballot(&conn, 2026, 1, 1, &missing)),
            "One or more quarterbacks do not exist"
        );

        conn.execute(
            "UPDATE poll_weeks SET closes_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-1 seconds')
             WHERE season = 2026 AND week = 1",
            [],
        )
        .unwrap();
        assert_eq!(
            detail(submit_ballot(&conn, 2026, 1, 1, &ids)),
            "Voting for this week has closed"
        );
        let votes: i64 = conn
            .query_row("SELECT count(*) FROM votes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(votes, 0);
    }

    #[test]
    fn rankings_score_and_track_movement() {
        let conn = setup();
        let ids = qb_ids(&conn, 16);
        let a: Vec<i64> = ids[..15].to_vec();
        let mut b = a.clone();
        b.swap(0, 1);

        // Previous existing week is 2026/1, not the missing 2026/2.
        submit_ballot(&conn, 2026, 1, 1, &a).unwrap();
        submit_ballot(&conn, 2026, 3, 1, &b).unwrap();
        submit_ballot(&conn, 2026, 3, 2, &b).unwrap();

        let r = rankings(&conn, 2026, 3).unwrap();
        assert_eq!(r.ballots, 2);
        assert_eq!(r.rankings.len(), 10);
        let top = &r.rankings[0];
        assert_eq!((top.quarterback.id, top.points, top.votes), (ids[1], 30, 2));
        assert_eq!(top.previous_rank, Some(2));
        assert_eq!(r.rankings[1].previous_rank, Some(1));

        let first = rankings(&conn, 2026, 1).unwrap();
        assert!(first.rankings.iter().all(|e| e.previous_rank.is_none()));

        let earliest = rankings(&conn, 2025, 18).unwrap();
        assert_eq!(earliest.ballots, 0);
        assert!(earliest.rankings.is_empty());
    }
}
