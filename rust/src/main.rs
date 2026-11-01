use anyhow::Result;

#[ntex::main]
async fn main() -> Result<()> {
    rust::db::migrate()?;
    rust::serve("127.0.0.1:8000").await?;
    Ok(())
}
