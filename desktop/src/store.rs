//! Per-device state the web app keeps in `localStorage`: unsaved ballot drafts
//! and the badge choice. It's best-effort in the same way: a missing or
//! unreadable file just means starting fresh.
//!
//! Reads and writes happen on the UI thread. The file is a few KB at most,
//! and doing it inline keeps writes in the order the edits happened.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::screen::poll::Lens;

#[derive(Default, Serialize, Deserialize)]
struct Data {
    #[serde(default)]
    lens: Option<Lens>,
    /// Keyed by `DraftKey`; values are quarterback ids in ballot order.
    #[serde(default)]
    drafts: HashMap<String, Vec<i64>>,
}

/// Includes the server and user so drafts never leak between accounts or
/// between a dev backend and prod.
pub struct DraftKey(String);

impl DraftKey {
    pub fn new(server: &str, user_id: i64, season: i64, week: i64) -> Self {
        Self(format!("{server}|{user_id}|{season}|{week}"))
    }
}

pub struct Store {
    path: Option<PathBuf>,
    data: Data,
}

impl Store {
    pub fn load() -> Self {
        let path = directories::ProjectDirs::from("xyz", "ryansheppard", "pollingjuegos")
            .map(|dirs| dirs.config_dir().join("state.json"));
        let data = path
            .as_ref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self { path, data }
    }

    pub fn lens(&self) -> Option<Lens> {
        self.data.lens
    }

    pub fn set_lens(&mut self, lens: Lens) {
        if self.data.lens != Some(lens) {
            self.data.lens = Some(lens);
            self.save();
        }
    }

    pub fn draft(&self, key: &DraftKey) -> Option<&[i64]> {
        self.data.drafts.get(&key.0).map(Vec::as_slice)
    }

    /// `None` clears the draft, e.g. once it matches the saved ballot.
    pub fn set_draft(&mut self, key: &DraftKey, ids: Option<Vec<i64>>) {
        let changed = match ids {
            Some(ids) => self.data.drafts.insert(key.0.clone(), ids.clone()) != Some(ids),
            None => self.data.drafts.remove(&key.0).is_some(),
        };
        if changed {
            self.save();
        }
    }

    fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };
        if let Err(error) = write_atomic(path, &self.data) {
            eprintln!("saving local state: {error:#}");
        }
    }
}

/// Writes to a temp file and renames it over the old one so a crash
/// mid-write can't leave a truncated file behind.
fn write_atomic(path: &PathBuf, data: &Data) -> Result<()> {
    let dir = path.parent().context("state path has no parent")?;
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(data)?)
        .with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}
