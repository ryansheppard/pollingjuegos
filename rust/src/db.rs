use rusqlite::{Connection, config::DbConfig};
use std::{
    cell::{Ref, RefCell},
    rc::Rc,
};

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("./migrations");
}

#[derive(Clone)]
pub struct Db(Rc<RefCell<Connection>>);

impl Db {
    pub fn new(conn: Connection) -> Self {
        Self(Rc::new(RefCell::new(conn)))
    }

    pub fn conn(&self) -> Ref<'_, Connection> {
        self.0.borrow()
    }
}

pub fn open_db(path: String) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    configure(&conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> rusqlite::Result<()> {
    // Without QPSG, SQLite marks joins filtered by a `?` to re-plan whenever
    // that parameter is rebound, and rusqlite clears bindings when a statement
    // returns to its cache, so every `prepare_cached` join was being reparsed and
    // replanned on every call (~18us vs ~2us). Value-dependent plans only help
    // with ANALYZE stats or bound LIKE patterns, which nothing here uses.
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_QPSG, true)?;
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = 5000;",
    )
}

pub fn migrate(path: String) -> anyhow::Result<()> {
    embedded::migrations::runner().run(&mut open_db(path)?)?;
    Ok(())
}

#[cfg(test)]
pub fn test_conn() -> Connection {
    let mut conn = test_conn_at(None);
    migrate_test_conn(&mut conn);
    conn
}

/// For testing a migration against rows written under an older schema:
/// `version` stops after that migration, `None` applies none.
#[cfg(test)]
pub fn test_conn_at(version: Option<u32>) -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    configure(&conn).unwrap();
    if let Some(v) = version {
        embedded::migrations::runner()
            .set_target(refinery::Target::Version(v.try_into().unwrap()))
            .run(&mut conn)
            .unwrap();
    }
    conn
}

#[cfg(test)]
pub fn migrate_test_conn(conn: &mut Connection) {
    embedded::migrations::runner().run(conn).unwrap();
}
