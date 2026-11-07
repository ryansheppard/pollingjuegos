use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};

use crate::error::{AppError, Result};
use api_types::{Ballot, BallotEntry, Quarterback, RankingEntry, Rankings, Team};

pub const BALLOT_SIZE: i64 = 15;
const RANKINGS_SIZE: i64 = 10;

/// Selects `q.id, q.name` plus the team columns `qb_from_row` expects; callers
/// must alias `quarterbacks` as `q` and LEFT JOIN `teams` as `t`.
const QB_COLUMNS: &str = "q.id AS qb_id, q.name AS qb_name,
    t.id AS team_id, t.name AS team_name, t.abbreviation AS team_abbreviation";

fn qb_from_row(row: &Row) -> rusqlite::Result<Quarterback> {
    let team_id: Option<i64> = row.get("team_id")?;
    Ok(Quarterback {
        id: row.get("qb_id")?,
        name: row.get("qb_name")?,
        team: match team_id {
            Some(id) => Some(Team {
                id,
                name: row.get("team_name")?,
                abbreviation: row.get("team_abbreviation")?,
            }),
            None => None,
        },
    })
}

pub const MAX_QB_PAGE: i64 = 100;

/// `search` is a case-insensitive substring match on QB name, team name, or team abbreviation.
pub fn list_quarterbacks(
    conn: &Connection,
    search: &str,
    team_id: Option<i64>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Quarterback>> {
    check_page(limit, offset)?;
    query_quarterbacks(conn, search, team_id, limit, offset)
}

pub fn check_page(limit: i64, offset: i64) -> Result<()> {
    if !(1..=MAX_QB_PAGE).contains(&limit) || offset < 0 {
        return Err(AppError::BadRequest(
            "limit must be 1-100 and offset nonnegative",
        ));
    }
    Ok(())
}

/// Every quarterback in `list_quarterbacks` order.
pub fn all_quarterbacks(conn: &Connection) -> Result<Vec<Quarterback>> {
    // SQLite treats a negative LIMIT as no limit.
    query_quarterbacks(conn, "", None, -1, 0)
}

fn query_quarterbacks(
    conn: &Connection,
    search: &str,
    team_id: Option<i64>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Quarterback>> {
    Ok(conn
        .prepare_cached(&format!(
            "SELECT {QB_COLUMNS}
             FROM quarterbacks q
             LEFT JOIN teams t ON t.id = q.team_id
             WHERE (?1 = ''
                    OR instr(lower(q.name), lower(?1)) > 0
                    OR instr(lower(t.name), lower(?1)) > 0
                    OR instr(lower(t.abbreviation), lower(?1)) > 0)
               AND (?2 IS NULL OR q.team_id = ?2)
             ORDER BY q.name, q.id
             LIMIT ?3 OFFSET ?4"
        ))?
        .query_map(params![search.trim(), team_id, limit, offset], qb_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

fn week_id(conn: &Connection, season: i64, week: i64) -> Result<i64> {
    conn.prepare_cached("SELECT id FROM poll_weeks WHERE season = ?1 AND week = ?2")?
        .query_row([season, week], |row| row.get(0))
        .optional()?
        .ok_or(AppError::NotFound)
}

pub fn get_ballot(conn: &Connection, season: i64, week: i64, voter_id: i64) -> Result<Ballot> {
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
    Ok(Ballot {
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
) -> Result<Ballot> {
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
    quarterback: Quarterback,
    points: i64,
    votes: i64,
}

/// Reads totals maintained by the `week_scores` triggers (migration V4), which
/// award AP-style points: #1 earns `BALLOT_SIZE` down to 1 point for last.
fn top_scores(conn: &Connection, poll_week_id: i64) -> Result<Vec<Score>> {
    Ok(conn
        .prepare_cached(&format!(
            "SELECT {QB_COLUMNS}, s.points, s.votes
             FROM week_scores s
             JOIN quarterbacks q ON q.id = s.quarterback_id
             LEFT JOIN teams t ON t.id = q.team_id
             WHERE s.poll_week_id = ?1
             ORDER BY s.points DESC, s.votes DESC, q.name, q.id
             LIMIT ?2"
        ))?
        .query_map(params![poll_week_id, RANKINGS_SIZE], |row| {
            Ok(Score {
                quarterback: qb_from_row(row)?,
                points: row.get("points")?,
                votes: row.get("votes")?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Movement compares with the closest earlier week that exists, not strictly week - 1.
pub fn rankings(conn: &Connection, season: i64, week: i64) -> Result<Rankings> {
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

    Ok(Rankings {
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

    fn add_voters(conn: &Connection, n: i64) -> Vec<i64> {
        (0..n)
            .map(|i| {
                conn.query_row(
                    "INSERT INTO users (discord_id, username) VALUES (?1, ?2) RETURNING id",
                    params![(100 + i).to_string(), format!("voter{i}")],
                    |r| r.get(0),
                )
                .unwrap()
            })
            .collect()
    }

    /// Recomputes with `BALLOT_SIZE` from Rust so the triggers' hardcoded points
    /// formula can't silently diverge from it.
    fn assert_week_scores_in_sync(conn: &Connection) {
        let rows = |sql: &str, p: &[&dyn rusqlite::ToSql]| -> Vec<(i64, i64, i64, i64)> {
            conn.prepare(sql)
                .unwrap()
                .query_map(p, |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap()
        };
        let expected = rows(
            "SELECT b.poll_week_id, v.quarterback_id, sum(?1 + 1 - v.rank), count(*)
             FROM ballots b JOIN votes v ON v.ballot_id = b.id
             GROUP BY 1, 2 ORDER BY 1, 2",
            &[&BALLOT_SIZE],
        );
        let actual = rows(
            "SELECT poll_week_id, quarterback_id, points, votes FROM week_scores ORDER BY 1, 2",
            &[],
        );
        assert_eq!(actual, expected);
    }

    fn detail(r: Result<Ballot>) -> &'static str {
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
    fn quarterbacks_include_team_and_paginate() {
        let conn = setup();
        let total: i64 = conn
            .query_row("SELECT count(*) FROM quarterbacks", [], |r| r.get(0))
            .unwrap();
        let first = list_quarterbacks(&conn, "", None, 10, 0).unwrap();
        assert_eq!(first.len(), 10);
        assert!(first.iter().all(|qb| qb.team.is_some()));
        assert!(first.windows(2).all(|w| w[0].name <= w[1].name));

        let rest = list_quarterbacks(&conn, "", None, MAX_QB_PAGE, 10).unwrap();
        assert_eq!(rest.len() as i64, total - 10);
        assert!(rest.iter().all(|qb| first.iter().all(|f| f.id != qb.id)));
        assert!(
            list_quarterbacks(&conn, "", None, 10, total)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn quarterbacks_search_and_filter() {
        let conn = setup();
        let names = |search: &str, team_id: Option<i64>| -> Vec<String> {
            list_quarterbacks(&conn, search, team_id, MAX_QB_PAGE, 0)
                .unwrap()
                .into_iter()
                .map(|qb| qb.name)
                .collect()
        };
        assert_eq!(names("  MAHOMES ", None), ["Patrick Mahomes"]);
        let by_abbr = names("kc", None);
        assert!(by_abbr.contains(&"Patrick Mahomes".to_owned()));
        let chiefs: i64 = conn
            .query_row("SELECT id FROM teams WHERE abbreviation = 'KC'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(names("", Some(chiefs)), names("Kansas City", None));
        assert!(names("", Some(999_999)).is_empty());
    }

    #[test]
    fn quarterbacks_reject_bad_pages() {
        let conn = setup();
        for (limit, offset) in [(0, 0), (MAX_QB_PAGE + 1, 0), (10, -1)] {
            assert!(matches!(
                list_quarterbacks(&conn, "", None, limit, offset),
                Err(AppError::BadRequest(_))
            ));
        }
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

    #[test]
    fn week_scores_track_every_write_path() {
        let conn = setup();
        let voters = add_voters(&conn, 4);
        let ids = qb_ids(&conn, 20);
        // Overlapping windows leave some QBs with only one or two votes, which
        // is where the votes = 1 removal path in the delete trigger matters.
        for (i, &voter) in voters.iter().enumerate() {
            for week in [1, 3] {
                submit_ballot(&conn, 2026, week, voter, &ids[i..i + 15]).unwrap();
            }
        }
        submit_ballot(&conn, 2025, 18, voters[0], &ids[5..20]).unwrap();
        assert_week_scores_in_sync(&conn);

        let mut reordered = ids[2..17].to_vec();
        reordered.reverse();
        submit_ballot(&conn, 2026, 1, voters[1], &reordered).unwrap();
        assert_week_scores_in_sync(&conn);

        let deletes = [
            "DELETE FROM votes WHERE ballot_id = (SELECT min(id) FROM ballots)",
            // Cascades from ballots used to drop the ballot before its votes.
            "DELETE FROM ballots WHERE id = (SELECT max(id) FROM ballots)",
            "DELETE FROM ballots WHERE voter_id = ?1",
            "DELETE FROM poll_weeks WHERE season = 2026 AND week = 3",
        ];
        for sql in deletes {
            let n = if sql.contains("?1") {
                conn.execute(sql, [voters[2]])
            } else {
                conn.execute(sql, [])
            }
            .unwrap();
            assert!(n > 0, "{sql} deleted nothing");
            assert_week_scores_in_sync(&conn);
        }

        conn.execute("DELETE FROM ballots", []).unwrap();
        let left: i64 = conn
            .query_row("SELECT count(*) FROM week_scores", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 0);
    }

    #[test]
    fn week_scores_backfill_existing_votes() {
        let mut conn = db::test_conn_at(Some(3));
        conn.execute_batch(
            "INSERT INTO users (id, discord_id, username) VALUES (1, '1', 'a'), (2, '2', 'b');
             INSERT INTO poll_weeks (id, season, week) VALUES (1, 2026, 1), (2, 2026, 2);
             INSERT INTO ballots (id, poll_week_id, voter_id) VALUES (1, 1, 1), (2, 1, 2), (3, 2, 1);",
        )
        .unwrap();
        let ids = qb_ids(&conn, 16);
        for (ballot, window) in [(1, &ids[..15]), (2, &ids[1..16]), (3, &ids[..15])] {
            for (rank, qb) in (1i64..).zip(window) {
                conn.execute(
                    "INSERT INTO votes (ballot_id, quarterback_id, rank) VALUES (?1, ?2, ?3)",
                    params![ballot, qb, rank],
                )
                .unwrap();
            }
        }

        db::migrate_test_conn(&mut conn);
        assert_week_scores_in_sync(&conn);
        let top = &rankings(&conn, 2026, 1).unwrap().rankings[0];
        // ids[1] is #2 then #1: 14 + 15.
        assert_eq!((top.quarterback.id, top.points, top.votes), (ids[1], 29, 2));
    }

    #[test]
    fn week_scores_reject_untracked_updates() {
        let conn = setup();
        let ids = qb_ids(&conn, 15);
        submit_ballot(&conn, 2026, 1, 1, &ids).unwrap();

        for sql in [
            "UPDATE votes SET rank = rank + 100",
            "UPDATE votes SET quarterback_id = (SELECT max(id) FROM quarterbacks) WHERE rank = 1",
            "UPDATE ballots SET poll_week_id = (SELECT id FROM poll_weeks WHERE season = 2026 AND week = 3)",
        ] {
            let err = conn.execute(sql, []).unwrap_err().to_string();
            assert!(
                err.contains("week_scores"),
                "{sql} should be rejected: {err}"
            );
        }
        // The upsert in submit_ballot rewrites voter_id; that must stay allowed.
        conn.execute("UPDATE ballots SET voter_id = voter_id", [])
            .unwrap();
        assert_week_scores_in_sync(&conn);
    }
}
