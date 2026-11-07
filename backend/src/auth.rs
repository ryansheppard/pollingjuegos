use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ntex::http::{HeaderMap, HttpMessage, Payload, header};
use ntex::web::{FromRequest, HttpRequest};
use rand::Rng;
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::db::Db;
use crate::error::AppError;

pub const SESSION_COOKIE: &str = "__Host-session";
pub const SESSION_TTL_SECS: i64 = 14 * 24 * 60 * 60;
/// Only has to outlive the browser's hop to the app's loopback listener.
pub const LOGIN_CODE_TTL_SECS: i64 = 60;

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
        .and_then(|c| hash_token(c.value()))
    else {
        return Ok(None);
    };
    lookup_user(&db.conn(), &token_hash)
}

/// Bearer sessions are the same rows as cookie sessions; only the transport differs.
pub fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

pub fn bearer_user(db: &Db, token: &str) -> rusqlite::Result<Option<AuthUser>> {
    match hash_token(token) {
        Some(token_hash) => lookup_user(&db.conn(), &token_hash),
        None => Ok(None),
    }
}

type TokenHash = [u8; 32];

pub fn new_token() -> (String, TokenHash) {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    (URL_SAFE_NO_PAD.encode(bytes), hash(&bytes))
}

pub fn hash_token(value: &str) -> Option<TokenHash> {
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

pub fn create_login_code(
    conn: &Connection,
    user_id: i64,
    challenge: &str,
) -> rusqlite::Result<String> {
    let (code, code_hash) = new_token();
    conn.execute(
        "INSERT INTO desktop_login_codes (code_hash, user_id, challenge, expires_at)
         VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%SZ', 'now', ?4))",
        params![
            code_hash,
            user_id,
            challenge,
            format!("+{LOGIN_CODE_TTL_SECS} seconds")
        ],
    )?;
    Ok(code)
}

/// Deletes the code even when it has expired or the caller's verifier turns out
/// wrong, so each code gets exactly one redemption attempt.
pub fn take_login_code(
    conn: &Connection,
    code_hash: &TokenHash,
) -> rusqlite::Result<Option<(i64, String)>> {
    let row: Option<(i64, String, bool)> = conn
        .prepare_cached(
            "DELETE FROM desktop_login_codes WHERE code_hash = ?1
             RETURNING user_id, challenge,
                 expires_at > strftime('%Y-%m-%dT%H:%M:%SZ', 'now') AS live",
        )?
        .query_row([code_hash], |row| {
            Ok((row.get("user_id")?, row.get("challenge")?, row.get("live")?))
        })
        .optional()?;
    Ok(row.and_then(|(user_id, challenge, live)| live.then_some((user_id, challenge))))
}

pub fn delete_expired_login_codes(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM desktop_login_codes WHERE expires_at <= strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
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
        lookup_user(conn, &hash_token(token).unwrap()).unwrap()
    }

    #[test]
    fn session_round_trip() {
        let (conn, user_id) = setup();
        let token = create_session(&conn, user_id).unwrap();
        let user = lookup(&conn, &token).unwrap();
        assert_eq!((user.id, user.discord_id.as_str()), (user_id, "123"));

        delete_session(&conn, &hash_token(&token).unwrap()).unwrap();
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
        assert!(hash_token("").is_none());
        assert!(hash_token("not base64!").is_none());
        assert!(hash_token(&URL_SAFE_NO_PAD.encode([0u8; 31])).is_none());
        assert!(hash_token(&new_token().0).is_some());
    }

    #[test]
    fn login_code_is_single_use() {
        let (conn, user_id) = setup();
        let code = create_login_code(&conn, user_id, "challenge").unwrap();
        let code_hash = hash_token(&code).unwrap();
        assert_eq!(
            take_login_code(&conn, &code_hash).unwrap(),
            Some((user_id, "challenge".to_owned()))
        );
        assert_eq!(take_login_code(&conn, &code_hash).unwrap(), None);
    }

    #[test]
    fn expired_login_codes_are_rejected_and_purged() {
        let (conn, user_id) = setup();
        let expire_all = || {
            conn.execute(
                "UPDATE desktop_login_codes
                 SET expires_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-1 seconds')",
                [],
            )
            .unwrap()
        };

        let code = create_login_code(&conn, user_id, "challenge").unwrap();
        expire_all();
        assert_eq!(
            take_login_code(&conn, &hash_token(&code).unwrap()).unwrap(),
            None
        );

        create_login_code(&conn, user_id, "challenge").unwrap();
        expire_all();
        assert_eq!(delete_expired_login_codes(&conn).unwrap(), 1);
    }

    #[test]
    fn upsert_refreshes_username() {
        let (conn, user_id) = setup();
        assert_eq!(upsert_user(&conn, "123", "renamed").unwrap(), user_id);
        let token = create_session(&conn, user_id).unwrap();
        assert_eq!(lookup(&conn, &token).unwrap().username, "renamed");
    }
}
