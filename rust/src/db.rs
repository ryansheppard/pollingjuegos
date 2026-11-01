use rusqlite::Connection;
use std::{
    cell::{Ref, RefCell},
    rc::Rc,
};

const DB_PATH: &str = "./db.sqlite3";

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

pub fn open_db() -> rusqlite::Result<Connection> {
    let conn = Connection::open(DB_PATH)?;
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = 5000;",
    )?;
    Ok(conn)
}

pub fn migrate() -> anyhow::Result<()> {
    embedded::migrations::runner().run(&mut open_db()?)?;
    Ok(())
}
