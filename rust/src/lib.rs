use ntex::{SharedCfg, web};
use std::{io, sync::Arc};

pub mod api;
pub mod config;
pub mod db;
pub mod error;
pub mod models;

#[derive(Clone)]
pub struct AppData {
    pub db: db::Db,
    pub cfg: Arc<config::Config>,
}

pub type St = web::AppState<AppData>;

pub async fn serve(cfg: Arc<config::Config>) -> io::Result<()> {
    let addr = cfg.bind_addr.clone();
    web::server_with_config(
        async move || -> io::Result<St> {
            let conn = db::open_db(cfg.db_path.clone()).map_err(io::Error::other)?;
            Ok(web::AppState::new(AppData {
                db: db::Db::new(conn),
                cfg: cfg.clone(),
            }))
        },
        async |_| web::App::<St>::new().configure(api::routes),
    )
    .bind(addr, SharedCfg::default())?
    .run()
    .await
}
