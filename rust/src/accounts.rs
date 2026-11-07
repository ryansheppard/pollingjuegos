use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use cookie::{Cookie, SameSite, time::Duration};
use ntex::http::{HttpMessage, header};
use ntex::web::{
    self, HttpRequest, HttpResponse,
    types::{Json, Query},
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::St;
use crate::auth::{self, SESSION_COOKIE, SESSION_TTL_SECS};
use crate::error::{AppError, Result};
use crate::middleware::same_origin;

const OAUTH_COOKIE: &str = "__Host-oauth";
const OAUTH_TTL_SECS: i64 = 10 * 60;
const LOGIN_PATH: &str = "/accounts/discord/login/";
const CALLBACK_PATH: &str = "/accounts/discord/login/callback/";
const DESKTOP_TOKEN_PATH: &str = "/accounts/desktop/token/";
const DESKTOP_LOGOUT_PATH: &str = "/accounts/desktop/logout/";
const DISCORD_AUTHORIZE: &str = "https://discord.com/oauth2/authorize";
const DISCORD_TOKEN: &str = "https://discord.com/api/oauth2/token";
const DISCORD_ME: &str = "https://discord.com/api/users/@me";
const DISCORD_REVOKE: &str = "https://discord.com/api/oauth2/token/revoke";

#[derive(Deserialize)]
struct NextQuery {
    next: Option<String>,
}

/// `desktop_port` and `desktop_challenge` come as a pair from a native app
/// doing an RFC 8252 loopback sign-in; browsers send neither.
#[derive(Deserialize)]
struct LoginQuery {
    next: Option<String>,
    desktop_port: Option<u16>,
    desktop_challenge: Option<String>,
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct OAuthState {
    state: String,
    next: String,
    pkce_verifier: String,
    desktop_port: Option<u16>,
    desktop_challenge: Option<String>,
}

struct DesktopLogin {
    port: u16,
    challenge: String,
}

impl OAuthState {
    fn desktop(&self) -> Option<DesktopLogin> {
        Some(DesktopLogin {
            port: self.desktop_port?,
            challenge: self.desktop_challenge.clone()?,
        })
    }
}

impl DesktopLogin {
    /// Fixed host and path so the only thing a crafted login link can choose is
    /// which port on the user's own machine receives the (PKCE-bound) code.
    fn redirect(&self, query: &[(&str, &str)]) -> HttpResponse {
        let query = serde_urlencoded::to_string(query).expect("string pairs encode");
        redirect(&format!("http://127.0.0.1:{}/callback?{query}", self.port))
    }
}

#[derive(Deserialize)]
struct DesktopTokenIn {
    code: String,
    code_verifier: String,
}

#[derive(Serialize)]
struct DesktopTokenOut {
    access_token: String,
    token_type: &'static str,
    expires_in: i64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct DiscordUser {
    id: String,
    username: String,
}

/// Only same-site relative paths; `//host` and `/\host` are treated as
/// absolute URLs by browsers and would make this an open redirect.
fn safe_next(next: Option<&str>) -> String {
    match next {
        Some(n)
            if n.starts_with('/')
                && !n.starts_with("//")
                && !n.starts_with("/\\")
                && n.bytes().all(|b| b.is_ascii_graphic()) =>
        {
            n.to_owned()
        }
        _ => "/".to_owned(),
    }
}

fn redirect(location: &str) -> HttpResponse {
    HttpResponse::Found()
        .header(header::LOCATION, location)
        .body("")
}

fn login_redirect(next: &str) -> HttpResponse {
    let query = serde_urlencoded::to_string([("next", next)]).expect("string pairs encode");
    redirect(&format!("{LOGIN_PATH}?{query}"))
}

fn host_cookie(name: &'static str, value: String, max_age_secs: i64) -> Cookie<'static> {
    Cookie::build((name, value))
        .path("/")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(Duration::seconds(max_age_secs))
        .build()
}

fn random_b64(n: usize) -> String {
    let mut bytes = vec![0u8; n];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// RFC 7636 S256: the challenge goes to Discord up front, the verifier only on
/// the token exchange, so an intercepted `code` alone can't be redeemed.
pub(crate) fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn redirect_uri(st: &St) -> String {
    format!("{}{CALLBACK_PATH}", st.cfg.public_origin)
}

async fn login_alias(_: &St, (): (), q: Query<NextQuery>) -> HttpResponse {
    login_redirect(&safe_next(q.next.as_deref()))
}

fn is_s256_challenge(challenge: &str) -> bool {
    URL_SAFE_NO_PAD
        .decode(challenge)
        .is_ok_and(|bytes| bytes.len() == 32)
}

async fn discord_login(st: &St, (): (), q: Query<LoginQuery>) -> Result<HttpResponse> {
    let LoginQuery {
        next,
        desktop_port,
        desktop_challenge,
    } = q.into_inner();
    match (desktop_port, &desktop_challenge) {
        (None, None) => {}
        (Some(port), Some(challenge)) if port != 0 && is_s256_challenge(challenge) => {}
        _ => {
            return Err(AppError::BadRequest(
                "desktop_port and an S256 desktop_challenge must be sent together",
            ));
        }
    }
    let oauth = OAuthState {
        state: random_b64(16),
        next: safe_next(next.as_deref()),
        pkce_verifier: random_b64(32),
        desktop_port,
        desktop_challenge,
    };
    let challenge = pkce_challenge(&oauth.pkce_verifier);
    let authorize = serde_urlencoded::to_string([
        ("response_type", "code"),
        ("client_id", &st.cfg.discord_client_id),
        ("scope", "identify"),
        ("redirect_uri", &redirect_uri(st)),
        ("state", &oauth.state),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
    ])
    .expect("string pairs encode");
    let cookie_value =
        URL_SAFE_NO_PAD.encode(serde_urlencoded::to_string(&oauth).expect("string fields encode"));

    Ok(HttpResponse::Found()
        .header(header::LOCATION, format!("{DISCORD_AUTHORIZE}?{authorize}"))
        .cookie(host_cookie(OAUTH_COOKIE, cookie_value, OAUTH_TTL_SECS))
        .body(""))
}

/// The state cookie is single-use, so it's cleared whether sign-in succeeds or not.
async fn discord_callback(
    st: &St,
    (): (),
    req: HttpRequest,
    q: Query<CallbackQuery>,
) -> HttpResponse {
    let mut res = complete_login(st, &req, q.into_inner())
        .await
        .unwrap_or_else(|e| e.to_response());
    if let Err(e) = res.add_cookie(host_cookie(OAUTH_COOKIE, String::new(), 0)) {
        eprintln!("clearing {OAUTH_COOKIE}: {e}");
    }
    res
}

async fn complete_login(st: &St, req: &HttpRequest, q: CallbackQuery) -> Result<HttpResponse> {
    let oauth: Option<OAuthState> = req
        .cookie(OAUTH_COOKIE)
        .and_then(|c| URL_SAFE_NO_PAD.decode(c.value()).ok())
        .and_then(|raw| serde_urlencoded::from_bytes(&raw).ok());
    if q.error.is_some() {
        // The user pressed Cancel on Discord's consent screen. A desktop app is
        // told too, so it can stop waiting on its loopback listener.
        return Ok(match oauth.as_ref().and_then(OAuthState::desktop) {
            Some(desktop) => desktop.redirect(&[("error", "access_denied")]),
            None => redirect("/"),
        });
    }
    let oauth = oauth.ok_or(AppError::BadRequest("Sign-in expired; try again."))?;
    let (Some(code), Some(state)) = (q.code, q.state) else {
        return Err(AppError::BadRequest("Missing code or state."));
    };
    if state != oauth.state {
        return Err(AppError::BadRequest("Sign-in state mismatch; try again."));
    }

    // The cookie isn't signed, so don't rely on it holding what we wrote.
    let next = safe_next(Some(&oauth.next));

    let discord_user = fetch_discord_user(st, &code, &oauth.pkce_verifier).await?;

    // No awaits past this point, so the connection borrow can't span a suspension.
    let conn = st.db.conn();
    if !auth::is_approved(&conn, &discord_user.id)? {
        return Err(AppError::NotApproved);
    }
    let user_id = auth::upsert_user(&conn, &discord_user.id, &discord_user.username)?;

    // The browser's own cookie session is left alone: signing in the app
    // shouldn't sign the browser in or out.
    if let Some(desktop) = oauth.desktop() {
        auth::delete_expired_login_codes(&conn)?;
        let code = auth::create_login_code(&conn, user_id, &desktop.challenge)?;
        return Ok(desktop.redirect(&[("code", &code)]));
    }

    if let Some(previous) = req
        .cookie(SESSION_COOKIE)
        .and_then(|c| auth::hash_token(c.value()))
    {
        auth::delete_session(&conn, &previous)?;
    }
    auth::delete_expired_sessions(&conn)?;
    let token = auth::create_session(&conn, user_id)?;

    Ok(HttpResponse::Found()
        .header(header::LOCATION, next)
        .cookie(host_cookie(SESSION_COOKIE, token, SESSION_TTL_SECS))
        .body(""))
}

async fn fetch_discord_user(st: &St, code: &str, pkce_verifier: &str) -> Result<DiscordUser> {
    let upstream = |e: &dyn std::fmt::Display| AppError::Upstream(e.to_string());

    let res = st
        .http
        .post(DISCORD_TOKEN)
        .basic_auth(
            &st.cfg.discord_client_id,
            Some(&st.cfg.discord_client_secret),
        )
        .send_form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &redirect_uri(st)),
            ("code_verifier", pkce_verifier),
        ])
        .await
        .map_err(|e| upstream(&e))?;
    if !res.status().is_success() {
        return Err(AppError::Upstream(format!(
            "token exchange returned {}",
            res.status()
        )));
    }
    let token: TokenResponse = res.json().await.map_err(|e| upstream(&e))?;

    let user = fetch_me(st, &token.access_token).await;
    revoke_in_background(st, token.access_token);
    user
}

async fn fetch_me(st: &St, access_token: &str) -> Result<DiscordUser> {
    let upstream = |e: &dyn std::fmt::Display| AppError::Upstream(e.to_string());
    let res = st
        .http
        .get(DISCORD_ME)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| upstream(&e))?;
    if !res.status().is_success() {
        return Err(AppError::Upstream(format!(
            "users/@me returned {}",
            res.status()
        )));
    }
    res.json().await.map_err(|e| upstream(&e))
}

/// The token is only needed for the one `users/@me` call; revoking it means a
/// leak can't be used for the rest of its ~7-day lifetime. Spawned so sign-in
/// doesn't wait on it, and failures are only logged since the session is ours.
fn revoke_in_background(st: &St, access_token: String) {
    let http = st.http.clone();
    let cfg = st.cfg.clone();
    ntex::rt::spawn(async move {
        let res = http
            .post(DISCORD_REVOKE)
            .basic_auth(&cfg.discord_client_id, Some(&cfg.discord_client_secret))
            .send_form(&[
                ("token", access_token.as_str()),
                ("token_type_hint", "access_token"),
            ])
            .await;
        match res {
            Ok(r) if r.status().is_success() => {}
            Ok(r) => eprintln!("revoking Discord token returned {}", r.status()),
            Err(e) => eprintln!("revoking Discord token: {e}"),
        }
    });
}

async fn logout(st: &St, (): (), req: HttpRequest) -> Result<HttpResponse> {
    if !same_origin(req.headers(), &st.cfg.public_origin) {
        return Err(AppError::CrossOrigin);
    }
    if let Some(token_hash) = req
        .cookie(SESSION_COOKIE)
        .and_then(|c| auth::hash_token(c.value()))
    {
        auth::delete_session(&st.db.conn(), &token_hash)?;
    }
    Ok(HttpResponse::SeeOther()
        .header(header::LOCATION, "/")
        .cookie(host_cookie(SESSION_COOKIE, String::new(), 0))
        .body(""))
}

/// No origin check: the response is only useful to whoever can read it, which
/// cross-site pages can't, and the code is bound to the app's PKCE verifier.
async fn desktop_token(st: &St, (): (), body: Json<DesktopTokenIn>) -> Result<HttpResponse> {
    let invalid = || AppError::BadRequest("Invalid or expired login code.");
    let code_hash = auth::hash_token(&body.code).ok_or_else(invalid)?;
    let conn = st.db.conn();
    let (user_id, challenge) = auth::take_login_code(&conn, &code_hash)?.ok_or_else(invalid)?;
    if pkce_challenge(&body.code_verifier) != challenge {
        return Err(invalid());
    }
    auth::delete_expired_sessions(&conn)?;
    let token = auth::create_session(&conn, user_id)?;
    Ok(crate::json(
        HttpResponse::Ok(),
        &DesktopTokenOut {
            access_token: token,
            token_type: "Bearer",
            expires_in: SESSION_TTL_SECS,
        },
    ))
}

/// Bearer-only, so like the `/api` scope it needs no origin check; cookie
/// sessions sign out through `logout`.
async fn desktop_logout(st: &St, (): (), req: HttpRequest) -> Result<HttpResponse> {
    let token_hash = auth::bearer_token(req.headers())
        .and_then(auth::hash_token)
        .ok_or(AppError::Unauthorized)?;
    auth::delete_session(&st.db.conn(), &token_hash)?;
    Ok(HttpResponse::NoContent().into())
}

/// Caddy's `forward_auth` hits this before serving any frontend file: 2xx lets
/// the request through, anything else is returned to the browser as-is.
async fn frontend_access(st: &St, (): (), req: HttpRequest) -> Result<HttpResponse> {
    Ok(match auth::session_user(&st.db, &req)? {
        Some(_) => HttpResponse::NoContent().into(),
        None => login_redirect("/"),
    })
}

pub fn routes(cfg: &mut web::ServiceConfig<St>) {
    cfg.service(web::resource("/accounts/login/").route(web::get().to_with_state(login_alias)))
        .service(web::resource(LOGIN_PATH).route(web::get().to_with_state(discord_login)))
        .service(web::resource(CALLBACK_PATH).route(web::get().to_with_state(discord_callback)))
        .service(web::resource("/accounts/logout/").route(web::post().to_with_state(logout)))
        .service(web::resource(DESKTOP_TOKEN_PATH).route(web::post().to_with_state(desktop_token)))
        .service(
            web::resource(DESKTOP_LOGOUT_PATH).route(web::post().to_with_state(desktop_logout)),
        )
        .service(web::resource("/auth/frontend/").route(web::get().to_with_state(frontend_access)));
}

#[cfg(test)]
mod tests {
    use super::{pkce_challenge, safe_next};

    #[test]
    fn pkce_challenge_matches_rfc_7636_vector() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn next_must_be_a_local_path() {
        assert_eq!(safe_next(Some("/weeks/2026/9?x=1")), "/weeks/2026/9?x=1");
        for bad in [
            None,
            Some(""),
            Some("https://evil.example"),
            Some("//evil.example"),
            Some("/\\evil.example"),
            Some("/a\r\nSet-Cookie: x"),
        ] {
            assert_eq!(safe_next(bad), "/", "{bad:?}");
        }
    }
}
