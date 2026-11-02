//! Serialized responses for endpoints whose data changes far less often than
//! it is read. Each worker owns one connection and one cache, so an entry is
//! valid exactly while that connection's view of the database is unchanged.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use ntex::util::Bytes;
use rusqlite::Connection;

use crate::nfl_stats::ByName;

/// Moves after any commit this connection can see: `data_version` covers
/// other connections (another worker, a hand-run `sqlite3`), and
/// `total_changes` covers this connection's own writes, which `data_version`
/// deliberately ignores.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DbVersion {
    data_version: i64,
    total_changes: u64,
}

impl DbVersion {
    pub fn of(conn: &Connection) -> rusqlite::Result<Self> {
        Ok(Self {
            data_version: conn
                .prepare_cached("PRAGMA data_version")?
                .query_row([], |row| row.get(0))?,
            total_changes: conn.total_changes(),
        })
    }
}

#[derive(Clone, Default)]
pub struct ResponseCache(Rc<RefCell<Entries>>);

#[derive(Default)]
struct Entries {
    quarterbacks: Option<Quarterbacks>,
    // Only successful lookups are stored, so keys are limited to weeks that exist.
    rankings: HashMap<(i64, i64), (DbVersion, Bytes)>,
}

struct Quarterbacks {
    version: DbVersion,
    stats: Arc<ByName>,
    items: Rc<[Bytes]>,
}

impl ResponseCache {
    /// One serialized quarterback per element, in list order, so a page is a
    /// slice joined into an array. Also rebuilt when the stats snapshot changes.
    pub fn quarterbacks<E>(
        &self,
        version: DbVersion,
        stats: &Arc<ByName>,
        build: impl FnOnce() -> Result<Vec<Bytes>, E>,
    ) -> Result<Rc<[Bytes]>, E> {
        if let Some(hit) = &self.0.borrow().quarterbacks
            && hit.version == version
            && Arc::ptr_eq(&hit.stats, stats)
        {
            return Ok(hit.items.clone());
        }
        let items: Rc<[Bytes]> = build()?.into();
        self.0.borrow_mut().quarterbacks = Some(Quarterbacks {
            version,
            stats: stats.clone(),
            items: items.clone(),
        });
        Ok(items)
    }

    pub fn rankings<E>(
        &self,
        season: i64,
        week: i64,
        version: DbVersion,
        build: impl FnOnce() -> Result<Bytes, E>,
    ) -> Result<Bytes, E> {
        if let Some((v, body)) = self.0.borrow().rankings.get(&(season, week))
            && *v == version
        {
            return Ok(body.clone());
        }
        let body = build()?;
        self.0
            .borrow_mut()
            .rankings
            .insert((season, week), (version, body.clone()));
        Ok(body)
    }
}

/// Joins pre-serialized JSON values into one array body.
pub fn json_array<'a>(items: impl IntoIterator<Item = &'a Bytes>) -> Vec<u8> {
    let mut body = vec![b'['];
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            body.push(b',');
        }
        body.extend_from_slice(item);
    }
    body.push(b']');
    body
}
