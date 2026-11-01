use rusqlite::Connection;
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
    let mut conn = Connection::open_in_memory().unwrap();
    configure(&conn).unwrap();
    embedded::migrations::runner().run(&mut conn).unwrap();
    conn
}
