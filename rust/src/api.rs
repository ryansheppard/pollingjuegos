use jiff::Timestamp;
use ntex::web::{
    self,
    types::{Json, Path, Query},
};
use serde::Deserialize;

use crate::St;
use crate::auth::AuthUser;
use crate::ballots;
use crate::error::Result;
use crate::middleware::RequireAuth;
use crate::models::{BallotIn, Me, QuarterbackWithStats, WeekOut};
use crate::nfl_stats::NflStats;

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
    let qbs: Vec<_> = ballots::list_quarterbacks(&st.db.conn(), &q, team_id, limit, offset)?
        .into_iter()
        .map(|quarterback| QuarterbackWithStats {
            stats: NflStats::stats_for(&stats, &quarterback.name),
            quarterback,
        })
        .collect();
    Ok(web::HttpResponse::Ok().json(&qbs))
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
            Ok(WeekOut {
                id: row.get("id")?,
                season: row.get("season")?,
                week: row.get("week")?,
                closes_at,
                is_closed: closes_at.is_some_and(|t| now >= t),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(web::HttpResponse::Ok().json(&weeks))
}

async fn my_ballot(
    st: &St,
    (): (),
    user: AuthUser,
    path: Path<(i64, i64)>,
) -> Result<web::HttpResponse> {
    let (season, week) = path.into_inner();
    let ballot = ballots::get_ballot(&st.db.conn(), season, week, user.id)?;
    Ok(web::HttpResponse::Ok().json(&ballot))
}

async fn submit_ballot(
    st: &St,
    (): (),
    user: AuthUser,
    path: Path<(i64, i64)>,
    body: Json<BallotIn>,
) -> Result<web::HttpResponse> {
    let (season, week) = path.into_inner();
    let ballot =
        ballots::submit_ballot(&st.db.conn(), season, week, user.id, &body.quarterback_ids)?;
    Ok(web::HttpResponse::Ok().json(&ballot))
}

async fn rankings(st: &St, (): (), path: Path<(i64, i64)>) -> Result<web::HttpResponse> {
    let (season, week) = path.into_inner();
    let rankings = ballots::rankings(&st.db.conn(), season, week)?;
    Ok(web::HttpResponse::Ok().json(&rankings))
}

async fn me(user: AuthUser) -> web::HttpResponse {
    web::HttpResponse::Ok().json(&Me { id: user.id })
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
