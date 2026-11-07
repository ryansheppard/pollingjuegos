use jiff::Timestamp;
use ntex::web::{
    self,
    types::{Json, Path, Query},
};
use serde::Deserialize;

use ntex::util::Bytes;

use crate::St;
use crate::auth::AuthUser;
use crate::ballots;
use crate::cache::{DbVersion, json_array};
use crate::error::{AppError, Result};
use crate::middleware::RequireAuth;
use crate::nfl_stats::NflStats;
use api_types::{BallotRequest, Me, Quarterback, QuarterbackWithStats, Week};

#[derive(Deserialize)]
struct QuarterbackQuery {
    #[serde(default)]
    q: String,
    team_id: Option<i64>,
    #[serde(default = "default_qb_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
}

fn default_qb_limit() -> i64 {
    50
}

async fn quarterbacks(
    st: &St,
    (): (),
    query: Query<QuarterbackQuery>,
) -> Result<web::HttpResponse> {
    let QuarterbackQuery {
        q,
        team_id,
        limit,
        offset,
    } = query.into_inner();
    let stats = st.nfl.snapshot();
    let with_stats = |quarterback: Quarterback| QuarterbackWithStats {
        stats: NflStats::stats_for(&stats, &quarterback.name),
        quarterback,
    };
    let conn = st.db.conn();

    // The frontend only ever pages through the unfiltered list.
    if q.trim().is_empty() && team_id.is_none() {
        ballots::check_page(limit, offset)?;
        let items = st.cache.quarterbacks(DbVersion::of(&conn)?, &stats, || {
            ballots::all_quarterbacks(&conn)?
                .into_iter()
                .map(|qb| Ok(Bytes::from(serde_json::to_vec(&with_stats(qb))?)))
                .collect::<Result<Vec<_>>>()
        })?;
        let page = items
            .iter()
            .skip(usize::try_from(offset).unwrap_or(usize::MAX))
            .take(usize::try_from(limit).unwrap_or(usize::MAX));
        return Ok(crate::json_body(web::HttpResponse::Ok(), json_array(page)));
    }

    let qbs: Vec<_> = ballots::list_quarterbacks(&conn, &q, team_id, limit, offset)?
        .into_iter()
        .map(with_stats)
        .collect();
    Ok(crate::json(web::HttpResponse::Ok(), &qbs))
}

async fn weeks(st: &St, (): ()) -> Result<web::HttpResponse> {
    let conn = st.db.conn();
    let mut stmt = conn.prepare_cached(
        "SELECT id, season, week, closes_at FROM poll_weeks ORDER BY season DESC, week DESC",
    )?;
    let now = Timestamp::now();
    let weeks = stmt
        .query_map([], |row| {
            let closes_at: Option<Timestamp> = row.get("closes_at")?;
            Ok(Week {
                id: row.get("id")?,
                season: row.get("season")?,
                week: row.get("week")?,
                closes_at,
                is_closed: closes_at.is_some_and(|t| now >= t),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(crate::json(web::HttpResponse::Ok(), &weeks))
}

async fn my_ballot(
    st: &St,
    (): (),
    user: AuthUser,
    path: Path<(i64, i64)>,
) -> Result<web::HttpResponse> {
    let (season, week) = path.into_inner();
    let ballot = ballots::get_ballot(&st.db.conn(), season, week, user.id)?;
    Ok(crate::json(web::HttpResponse::Ok(), &ballot))
}

async fn submit_ballot(
    st: &St,
    (): (),
    user: AuthUser,
    path: Path<(i64, i64)>,
    body: Json<BallotRequest>,
) -> Result<web::HttpResponse> {
    let (season, week) = path.into_inner();
    let ballot =
        ballots::submit_ballot(&st.db.conn(), season, week, user.id, &body.quarterback_ids)?;
    Ok(crate::json(web::HttpResponse::Ok(), &ballot))
}

async fn rankings(st: &St, (): (), path: Path<(i64, i64)>) -> Result<web::HttpResponse> {
    let (season, week) = path.into_inner();
    let conn = st.db.conn();
    let body = st.cache.rankings(season, week, DbVersion::of(&conn)?, || {
        Ok::<_, AppError>(Bytes::from(serde_json::to_vec(&ballots::rankings(
            &conn, season, week,
        )?)?))
    })?;
    Ok(crate::json_body(web::HttpResponse::Ok(), body))
}

async fn me(user: AuthUser) -> web::HttpResponse {
    crate::json(web::HttpResponse::Ok(), &Me { id: user.id })
}

pub fn routes(cfg: &mut web::ServiceConfig<St>) {
    cfg.service(
        web::scope("/api")
            .middleware(RequireAuth)
            .service(web::resource("/quarterbacks").to_with_state(quarterbacks))
            .service(web::resource("/weeks").to_with_state(weeks))
            .service(
                web::resource("/weeks/{season}/{week}/ballot")
                    .route(web::get().to_with_state(my_ballot))
                    .route(web::put().to_with_state(submit_ballot)),
            )
            .service(
                web::resource("/weeks/{season}/{week}/rankings")
                    .route(web::get().to_with_state(rankings)),
            )
            .service(web::resource("/me").to(me)),
    );
}
