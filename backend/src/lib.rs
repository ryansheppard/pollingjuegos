use ntex::{SharedCfg, web};
use std::{io, sync::Arc};

pub mod accounts;
pub mod api;
pub mod auth;
pub mod ballots;
pub mod cache;
pub mod config;
pub mod db;
pub mod error;
pub mod middleware;
pub mod nfl_stats;

#[cfg(test)]
mod http_tests;

#[derive(Clone)]
pub struct AppData {
    pub db: db::Db,
    pub cfg: Arc<config::Config>,
    pub http: ntex::client::Client,
    pub nfl: nfl_stats::NflStats,
    pub cache: cache::ResponseCache,
}

pub type St = web::AppState<AppData>;

/// Use instead of ntex's `.json()`: that streams serde_json into a `BytesMut`
/// through `io::Write`, which profiled at ~19% of `/api/quarterbacks`' CPU,
/// while serializing into a `Vec` first raised its throughput ~25%.
pub fn json<T: serde::Serialize + ?Sized>(
    res: web::HttpResponseBuilder,
    value: &T,
) -> web::HttpResponse {
    match serde_json::to_vec(value) {
        Ok(body) => json_body(res, body),
        Err(e) => {
            eprintln!("serializing response: {e}");
            web::HttpResponse::InternalServerError().into()
        }
    }
}

/// For a body that is already serialized JSON.
pub fn json_body(
    mut res: web::HttpResponseBuilder,
    body: impl Into<ntex::http::body::Body>,
) -> web::HttpResponse {
    res.content_type("application/json").body(body)
}

pub fn routes(cfg: &mut web::ServiceConfig<St>) {
    api::routes(cfg);
    accounts::routes(cfg);
}

pub async fn serve(cfg: Arc<config::Config>) -> io::Result<()> {
    let addr = cfg.bind_addr.clone();
    let nfl = nfl_stats::NflStats::default();
    // Runs on the main runtime, which only accepts connections, so workers
    // serve requests (with `stats: null`) while the first load is in flight.
    ntex::rt::spawn(nfl.clone().refresh_forever(ntex::client::Client::new()));
    web::server_with_config(
        async move || -> io::Result<St> {
            let conn = db::open_db(cfg.db_path.clone()).map_err(io::Error::other)?;
            Ok(web::AppState::new(AppData {
                db: db::Db::new(conn),
                cfg: cfg.clone(),
                http: ntex::client::Client::new(),
                nfl: nfl.clone(),
                cache: cache::ResponseCache::default(),
            }))
        },
        async |_| web::App::<St>::new().configure(routes),
    )
    .bind(addr, SharedCfg::default())?
    .run()
    .await
}
