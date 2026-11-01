use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use cookie::{Cookie, SameSite, time::Duration};
use ntex::http::{HttpMessage, header};
use ntex::web::{self, HttpRequest, HttpResponse, types::Query};
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::St;
use crate::auth::{self, SESSION_COOKIE, SESSION_TTL_SECS};
use crate::error::{AppError, Result};
use crate::middleware::same_origin;

const OAUTH_COOKIE: &str = "__Host-oauth";
const OAUTH_TTL_SECS: i64 = 10 * 60;
const LOGIN_PATH: &str = "/accounts/discord/login/";
const CALLBACK_PATH: &str = "/accounts/discord/login/callback/";
const DISCORD_AUTHORIZE: &str = "https://discord.com/oauth2/authorize";
const DISCORD_TOKEN: &str = "https://discord.com/api/oauth2/token";
const DISCORD_ME: &str = "https://discord.com/api/users/@me";

#[derive(Deserialize)]
struct NextQuery {
    next: Option<String>,
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

fn redirect_uri(st: &St) -> String {
    format!("{}{CALLBACK_PATH}", st.cfg.public_origin)
}

async fn login_alias(_: &St, (): (), q: Query<NextQuery>) -> HttpResponse {
    login_redirect(&safe_next(q.next.as_deref()))
}

async fn discord_login(st: &St, (): (), q: Query<NextQuery>) -> HttpResponse {
    let oauth = OAuthState {
        state: random_b64(16),
        next: safe_next(q.next.as_deref()),
    };
    let authorize = serde_urlencoded::to_string([
        ("response_type", "code"),
        ("client_id", &st.cfg.discord_client_id),
        ("scope", "identify"),
        ("redirect_uri", &redirect_uri(st)),
        ("state", &oauth.state),
    ])
    .expect("string pairs encode");
    let cookie_value =
        URL_SAFE_NO_PAD.encode(serde_urlencoded::to_string(&oauth).expect("string fields encode"));

    HttpResponse::Found()
        .header(header::LOCATION, format!("{DISCORD_AUTHORIZE}?{authorize}"))
        .cookie(host_cookie(OAUTH_COOKIE, cookie_value, OAUTH_TTL_SECS))
        .body("")
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
    if q.error.is_some() {
        // The user pressed Cancel on Discord's consent screen.
        return Ok(redirect("/"));
    }
    let oauth: OAuthState = req
        .cookie(OAUTH_COOKIE)
        .and_then(|c| URL_SAFE_NO_PAD.decode(c.value()).ok())
        .and_then(|raw| serde_urlencoded::from_bytes(&raw).ok())
        .ok_or(AppError::BadRequest("Sign-in expired; try again."))?;
    let (Some(code), Some(state)) = (q.code, q.state) else {
        return Err(AppError::BadRequest("Missing code or state."));
    };
    if state != oauth.state {
        return Err(AppError::BadRequest("Sign-in state mismatch; try again."));
    }

    let discord_user = fetch_discord_user(st, &code).await?;

    // No awaits past this point, so the connection borrow can't span a suspension.
    let conn = st.db.conn();
    if !auth::is_approved(&conn, &discord_user.id)? {
        return Err(AppError::NotApproved);
    }
    let user_id = auth::upsert_user(&conn, &discord_user.id, &discord_user.username)?;
    let token = auth::create_session(&conn, user_id)?;

    Ok(HttpResponse::Found()
        .header(header::LOCATION, oauth.next)
        .cookie(host_cookie(SESSION_COOKIE, token, SESSION_TTL_SECS))
        .body(""))
}

async fn fetch_discord_user(st: &St, code: &str) -> Result<DiscordUser> {
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

    let res = st
        .http
        .get(DISCORD_ME)
        .bearer_auth(&token.access_token)
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

async fn logout(st: &St, (): (), req: HttpRequest) -> Result<HttpResponse> {
    if !same_origin(req.headers(), &st.cfg.public_origin) {
        return Err(AppError::CrossOrigin);
    }
    if let Some(token_hash) = req
        .cookie(SESSION_COOKIE)
        .and_then(|c| auth::hash_cookie(c.value()))
    {
        auth::delete_session(&st.db.conn(), &token_hash)?;
    }
    Ok(HttpResponse::SeeOther()
        .header(header::LOCATION, "/")
        .cookie(host_cookie(SESSION_COOKIE, String::new(), 0))
        .body(""))
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
        .service(web::resource("/auth/frontend/").route(web::get().to_with_state(frontend_access)));
}

#[cfg(test)]
mod tests {
    use super::safe_next;

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
