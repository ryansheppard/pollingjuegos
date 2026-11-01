use jiff::Timestamp;
use ntex::web;

use crate::St;
use crate::error::Result;
use crate::models::{Me, QuarterbackOut, WeekOut};

async fn quarterbacks(st: &St, (): ()) -> Result<web::HttpResponse> {
    let conn = st.db.conn();
    let mut stmt =
        conn.prepare_cached("SELECT id, name, team_id FROM quarterbacks ORDER BY name")?;
    let qbs = stmt
        .query_map([], |row| {
            Ok(QuarterbackOut {
                id: row.get("id")?,
                name: row.get("name")?,
                team_id: row.get("team_id")?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(web::HttpResponse::Ok().json(&qbs))
}

async fn weeks(st: &St, (): ()) -> Result<web::HttpResponse> {
    let conn = st.db.conn();
    let mut stmt = conn.prepare_cached(
        "SELECT id, season, week, closes_at FROM poll_weeks ORDER BY closes_at DESC",
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

async fn me() -> web::HttpResponse {
    web::HttpResponse::Ok().json(&Me { id: 1 })
}

pub fn routes(cfg: &mut web::ServiceConfig<St>) {
    cfg.service(
        web::scope("/api")
            .service(web::resource("/quarterbacks").to_with_state(quarterbacks))
            .service(web::resource("/weeks").to_with_state(weeks))
            .service(web::resource("/me").to(me)),
    );
}
