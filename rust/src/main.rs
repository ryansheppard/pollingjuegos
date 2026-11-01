use anyhow::Result;
use std::sync::Arc;

#[ntex::main]
async fn main() -> Result<()> {
    let cfg = Arc::new(rust::config::Config::from_env()?);
    rust::db::migrate(cfg.db_path.clone())?;
    rust::serve(cfg).await?;
    Ok(())
}
