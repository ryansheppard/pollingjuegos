use ntex::{SharedCfg, web};
use std::io;

pub mod api;
pub mod db;
pub mod error;
pub mod models;

pub type St = web::AppState<db::Db>;

pub async fn serve(addr: &str) -> io::Result<()> {
    web::server_with_config(
        async || -> io::Result<St> {
            let conn = db::open_db().map_err(io::Error::other)?;
            Ok(web::AppState::new(db::Db::new(conn)))
        },
        async |_| web::App::<St>::new().configure(api::routes),
    )
    .bind(addr, SharedCfg::default())?
    .run()
    .await
}
