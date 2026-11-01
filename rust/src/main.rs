use std::{cell::RefCell, io, rc::Rc};

use anyhow::Result;
use jiff::Timestamp;
use ntex::{SharedCfg, web};
use rusqlite::Connection;
use serde::Serialize;

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("./migrations");
}

#[derive(Clone)]
struct Db(Rc<RefCell<Connection>>);

type St = web::AppState<Db>;

#[derive(Serialize)]
struct QuarterbackOut {
    id: i64,
    name: String,
    team_id: Option<i64>,
}

#[derive(Serialize)]
struct WeekOut {
    id: i64,
    season: i64,
    week: i64,
    closes_at: Option<Timestamp>,
    is_closed: bool,
}

const DB_PATH: &str = "./db.sqlite3";

fn open_db() -> rusqlite::Result<Connection> {
    let conn = Connection::open(DB_PATH)?;
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = 5000;",
    )?;
    Ok(conn)
}

async fn quarterbacks(st: &St, (): ()) -> web::HttpResponse {
    let result = (|| {
        let conn = st.0.borrow();
        let mut stmt =
            conn.prepare_cached("SELECT id, name, team_id FROM quarterbacks ORDER BY name")?;
        stmt.query_map([], |row| {
            Ok(QuarterbackOut {
                id: row.get("id")?,
                name: row.get("name")?,
                team_id: row.get("team_id")?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
    })();

    match result {
        Ok(qbs) => web::HttpResponse::Ok().json(&qbs),
        Err(_) => web::HttpResponse::InternalServerError().into(),
    }
}

async fn weeks(st: &St, (): ()) -> web::HttpResponse {
    let result = (|| {
        let conn = st.0.borrow();
        let mut stmt = conn.prepare_cached(
            "SELECT id, season, week, closes_at FROM poll_weeks ORDER BY closes_at DESC",
        )?;
        stmt.query_map([], |row| {
            let closes_at: Option<Timestamp> = row.get("closes_at")?;
            let is_closed = closes_at.is_some_and(|t| Timestamp::now() >= t);
            Ok(WeekOut {
                id: row.get("id")?,
                season: row.get("season")?,
                week: row.get("week")?,
                closes_at,
                is_closed,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
    })();

    match result {
        Ok(qbs) => web::HttpResponse::Ok().json(&qbs),
        Err(_) => web::HttpResponse::InternalServerError().into(),
    }
}

#[derive(Serialize)]
struct Me {
    id: i64,
}

async fn me() -> web::HttpResponse {
    web::HttpResponse::Ok().json(&Me { id: 1 })
}

#[ntex::main]
async fn main() -> Result<()> {
    embedded::migrations::runner().run(&mut open_db()?)?;

    web::server_with_config(
        async || -> io::Result<St> {
            let conn = open_db().map_err(io::Error::other)?;
            Ok(web::AppState::new(Db(Rc::new(RefCell::new(conn)))))
        },
        async |_| {
            web::App::<St>::new()
                .service(web::resource("/api/quarterbacks").to_with_state(quarterbacks))
                .service(web::resource("/api/weeks").to_with_state(weeks))
                .service(web::resource("/api/me").to(me))
        },
    )
    .bind("127.0.0.1:8000", SharedCfg::default())?
    .run()
    .await?;

    Ok(())
}
