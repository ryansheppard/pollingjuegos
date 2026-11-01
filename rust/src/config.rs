use anyhow::{Context, Result, bail};
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: String,
    pub db_path: String,
    /// Scheme and host with no trailing slash, e.g. `https://pj.ryansheppard.xyz`.
    /// Used to build the OAuth redirect URI and to check `Origin` on writes.
    pub public_origin: String,
    pub discord_client_id: String,
    pub discord_client_secret: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let public_origin = required("PUBLIC_ORIGIN")?;
        if !(public_origin.starts_with("https://") || public_origin.starts_with("http://"))
            || public_origin.ends_with('/')
        {
            bail!("PUBLIC_ORIGIN must be scheme://host[:port] with no trailing slash");
        }

        Ok(Self {
            bind_addr: optional("BIND_ADDR", "127.0.0.1:8000")?,
            db_path: optional("DB_PATH", "./db.sqlite3")?,
            public_origin,
            discord_client_id: required("DISCORD_CLIENT_ID")?,
            discord_client_secret: required("DISCORD_CLIENT_SECRET")?,
        })
    }
}

fn required(key: &str) -> Result<String> {
    match env::var(key) {
        Ok(v) if !v.is_empty() => Ok(v),
        Ok(_) | Err(env::VarError::NotPresent) => bail!("{key} must be set"),
        Err(e) => Err(e).with_context(|| format!("reading {key}")),
    }
}

fn optional(key: &str, default: &str) -> Result<String> {
    match env::var(key) {
        Ok(v) if !v.is_empty() => Ok(v),
        Ok(_) | Err(env::VarError::NotPresent) => Ok(default.to_owned()),
        Err(e) => Err(e).with_context(|| format!("reading {key}")),
    }
}
