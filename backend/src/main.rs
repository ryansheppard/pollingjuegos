use anyhow::Result;
use std::sync::Arc;

#[ntex::main]
async fn main() -> Result<()> {
    let cfg = Arc::new(backend::config::Config::from_env()?);
    backend::db::migrate(cfg.db_path.clone())?;
    backend::auth::delete_expired_sessions(&backend::db::open_db(cfg.db_path.clone())?)?;
    backend::serve(cfg).await?;
    Ok(())
}
