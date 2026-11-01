use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ntex::http::{HttpMessage, Payload};
use ntex::web::{FromRequest, HttpRequest};
use rand::Rng;
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::db::Db;
use crate::error::AppError;

pub const SESSION_COOKIE: &str = "__Host-session";
pub const SESSION_TTL_SECS: i64 = 14 * 24 * 60 * 60;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: i64,
    pub discord_id: String,
    pub username: String,
}

/// Only resolves inside a scope wrapped by `RequireAuth`; elsewhere it is
/// always 401, so a handler can't accidentally run unauthenticated.
impl<St> FromRequest<St> for AuthUser {
    type Error = AppError;

    async fn from_request(_: &St, req: &HttpRequest, _: &mut Payload) -> Result<Self, AppError> {
        req.extensions()
            .get::<AuthUser>()
            .cloned()
            .ok_or(AppError::Unauthorized)
    }
}

/// The connection borrow ends before this returns, so callers can `.await`
/// afterwards without holding the `RefCell`.
pub fn session_user(db: &Db, req: &impl HttpMessage) -> rusqlite::Result<Option<AuthUser>> {
    let Some(token_hash) = req
        .cookie(SESSION_COOKIE)
        .and_then(|c| hash_cookie(c.value()))
    else {
        return Ok(None);
    };
    lookup_user(&db.conn(), &token_hash)
}

type TokenHash = [u8; 32];

pub fn new_token() -> (String, TokenHash) {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    (URL_SAFE_NO_PAD.encode(bytes), hash(&bytes))
}

pub fn hash_cookie(value: &str) -> Option<TokenHash> {
    let bytes: [u8; 32] = URL_SAFE_NO_PAD.decode(value).ok()?.try_into().ok()?;
    Some(hash(&bytes))
}

fn hash(bytes: &[u8]) -> TokenHash {
    Sha256::digest(bytes).into()
}

pub fn create_session(conn: &Connection, user_id: i64) -> rusqlite::Result<String> {
    let (token, token_hash) = new_token();
    conn.execute(
        "INSERT INTO sessions (token_hash, user_id, expires_at)
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ', 'now', ?3))",
        params![token_hash, user_id, format!("+{SESSION_TTL_SECS} seconds")],
    )?;
    Ok(token)
}

pub fn lookup_user(
    conn: &Connection,
    token_hash: &TokenHash,
) -> rusqlite::Result<Option<AuthUser>> {
    conn.prepare_cached(
        "SELECT u.id, u.discord_id, u.username
         FROM sessions s
         JOIN users u ON u.id = s.user_id
         JOIN approved_discord_users a ON a.discord_id = u.discord_id AND a.enabled = 1
         WHERE s.token_hash = ?1
           AND s.expires_at > strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
    )?
    .query_row([token_hash], |row| {
        Ok(AuthUser {
            id: row.get("id")?,
            discord_id: row.get("discord_id")?,
            username: row.get("username")?,
        })
    })
    .optional()
}

pub fn delete_session(conn: &Connection, token_hash: &TokenHash) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM sessions WHERE token_hash = ?1", [token_hash])?;
    Ok(())
}

pub fn delete_expired_sessions(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM sessions WHERE expires_at <= strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
        [],
    )
}

pub fn is_approved(conn: &Connection, discord_id: &str) -> rusqlite::Result<bool> {
    conn.prepare_cached(
        "SELECT EXISTS (SELECT 1 FROM approved_discord_users WHERE discord_id = ?1 AND enabled = 1)",
    )?
    .query_row([discord_id], |row| row.get(0))
}

pub fn upsert_user(conn: &Connection, discord_id: &str, username: &str) -> rusqlite::Result<i64> {
    conn.prepare_cached(
        "INSERT INTO users (discord_id, username) VALUES (?1, ?2)
         ON CONFLICT (discord_id) DO UPDATE SET username = excluded.username
         RETURNING id",
    )?
    .query_row([discord_id, username], |row| row.get(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn setup() -> (Connection, i64) {
        let conn = db::test_conn();
        conn.execute(
            "INSERT INTO approved_discord_users (discord_id) VALUES ('123')",
            [],
        )
        .unwrap();
        let user_id = upsert_user(&conn, "123", "ryan").unwrap();
        (conn, user_id)
    }

    fn lookup(conn: &Connection, token: &str) -> Option<AuthUser> {
        lookup_user(conn, &hash_cookie(token).unwrap()).unwrap()
    }

    #[test]
    fn session_round_trip() {
        let (conn, user_id) = setup();
        let token = create_session(&conn, user_id).unwrap();
        let user = lookup(&conn, &token).unwrap();
        assert_eq!((user.id, user.discord_id.as_str()), (user_id, "123"));

        delete_session(&conn, &hash_cookie(&token).unwrap()).unwrap();
        assert!(lookup(&conn, &token).is_none());
    }

    #[test]
    fn revoked_user_loses_session() {
        let (conn, user_id) = setup();
        let token = create_session(&conn, user_id).unwrap();
        conn.execute("UPDATE approved_discord_users SET enabled = 0", [])
            .unwrap();
        assert!(lookup(&conn, &token).is_none());
        assert!(!is_approved(&conn, "123").unwrap());
    }

    #[test]
    fn expired_session_is_rejected_and_purged() {
        let (conn, user_id) = setup();
        let token = create_session(&conn, user_id).unwrap();
        conn.execute(
            "UPDATE sessions SET expires_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-1 seconds')",
            [],
        )
        .unwrap();
        assert!(lookup(&conn, &token).is_none());
        assert_eq!(delete_expired_sessions(&conn).unwrap(), 1);
    }

    #[test]
    fn malformed_cookies_are_rejected() {
        assert!(hash_cookie("").is_none());
        assert!(hash_cookie("not base64!").is_none());
        assert!(hash_cookie(&URL_SAFE_NO_PAD.encode([0u8; 31])).is_none());
        assert!(hash_cookie(&new_token().0).is_some());
    }

    #[test]
    fn upsert_refreshes_username() {
        let (conn, user_id) = setup();
        assert_eq!(upsert_user(&conn, "123", "renamed").unwrap(), user_id);
        let token = create_session(&conn, user_id).unwrap();
        assert_eq!(lookup(&conn, &token).unwrap().username, "renamed");
    }
}
