//! Everything here blocks (network, loopback listener, OS keyring), so the UI
//! only calls it from a worker thread.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use api_types::{
    Ballot, BallotRequest, DesktopTokenRequest, DesktopTokenResponse, Me, QuarterbackWithStats,
    Rankings, Week,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::Rng;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use ureq::http;

use crate::loopback::{self, Callback};

const KEYRING_SERVICE: &str = "pollingjuegos";
/// Generous because the user may have to log in to Discord first; the server's
/// OAuth state cookie lasts 10 minutes, so waiting longer than that is pointless.
const BROWSER_TIMEOUT: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone)]
pub struct Session {
    pub token: String,
    pub user_id: i64,
}

/// The server's maximum page size; a shorter page means we've reached the end.
const QUARTERBACK_PAGE: usize = 100;

pub struct Client {
    server: String,
    agent: ureq::Agent,
}

impl Client {
    pub fn new(server: String) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
            .into();
        Self { server, agent }
    }

    pub fn server(&self) -> &str {
        &self.server
    }

    /// Keyed by server so a dev backend's token never gets sent to prod.
    fn keyring_entry(&self) -> Result<keyring::Entry> {
        keyring::Entry::new(KEYRING_SERVICE, &self.server).context("opening OS keyring")
    }

    /// `None` when there's no stored token or the server no longer accepts it.
    pub fn restore(&self) -> Result<Option<Session>> {
        let entry = self.keyring_entry()?;
        let token = match entry.get_password() {
            Ok(token) => token,
            Err(keyring::Error::NoEntry) => return Ok(None),
            Err(e) => return Err(e).context("reading saved session"),
        };
        Ok(self.me(&token)?.map(|user_id| Session { token, user_id }))
    }

    pub fn sign_in(&self) -> Result<Session> {
        let verifier = random_b64(32);
        let (listener, port) = loopback::bind().context("starting loopback listener")?;
        let query = serde_urlencoded::to_string([
            ("desktop_port", port.to_string()),
            ("desktop_challenge", pkce_challenge(&verifier)),
        ])?;
        webbrowser::open(&format!("{}/accounts/discord/login/?{query}", self.server))
            .context("opening browser")?;

        let code = match loopback::wait_for_callback(&listener, BROWSER_TIMEOUT)? {
            Some(Callback::Code(code)) => code,
            Some(Callback::Denied) => bail!("Sign-in was cancelled."),
            None => bail!("Timed out waiting for the browser."),
        };
        let token = self
            .agent
            .post(format!("{}/accounts/desktop/token/", self.server))
            .send_json(DesktopTokenRequest {
                code,
                code_verifier: verifier,
            })
            .context("exchanging login code")?
            .body_mut()
            .read_json::<DesktopTokenResponse>()?
            .access_token;

        let user_id = self
            .me(&token)?
            .context("server rejected the new session")?;
        self.keyring_entry()?
            .set_password(&token)
            .context("saving session")?;
        Ok(Session { token, user_id })
    }

    /// Forgets the local token first so the app is signed out even if the
    /// server can't be reached.
    pub fn sign_out(&self, token: &str) -> Result<()> {
        forget(&self.keyring_entry()?)?;
        self.agent
            .post(format!("{}/accounts/desktop/logout/", self.server))
            .header("Authorization", format!("Bearer {token}"))
            .send_empty()
            .context("ending session on server")?;
        Ok(())
    }

    fn me(&self, token: &str) -> Result<Option<i64>> {
        Ok(self.get::<Me>(token, "/me")?.map(|me| me.id))
    }

    pub fn weeks(&self, token: &str) -> Result<Option<Vec<Week>>> {
        self.get(token, "/weeks")
    }

    pub fn ballot(&self, token: &str, season: i64, week: i64) -> Result<Option<Ballot>> {
        self.get(token, &format!("/weeks/{season}/{week}/ballot"))
    }

    pub fn rankings(&self, token: &str, season: i64, week: i64) -> Result<Option<Rankings>> {
        self.get(token, &format!("/weeks/{season}/{week}/rankings"))
    }

    pub fn quarterbacks(&self, token: &str) -> Result<Option<Vec<QuarterbackWithStats>>> {
        let mut all = Vec::new();
        loop {
            let path = format!(
                "/quarterbacks?limit={QUARTERBACK_PAGE}&offset={}",
                all.len()
            );
            let Some(page) = self.get::<Vec<QuarterbackWithStats>>(token, &path)? else {
                return Ok(None);
            };
            let done = page.len() < QUARTERBACK_PAGE;
            all.extend(page);
            if done {
                return Ok(Some(all));
            }
        }
    }

    pub fn save_ballot(
        &self,
        token: &str,
        season: i64,
        week: i64,
        quarterback_ids: Vec<i64>,
    ) -> Result<Option<Ballot>> {
        let path = format!("/weeks/{season}/{week}/ballot");
        let res = self
            .agent
            .put(format!("{}/api{path}", self.server))
            .config()
            .http_status_as_error(false)
            .build()
            .header("Authorization", format!("Bearer {token}"))
            .send_json(BallotRequest { quarterback_ids })
            .with_context(|| format!("PUT /api{path}"))?;
        self.accept(res)
    }

    fn get<T: DeserializeOwned>(&self, token: &str, path: &str) -> Result<Option<T>> {
        let res = self
            .agent
            .get(format!("{}/api{path}", self.server))
            .config()
            .http_status_as_error(false)
            .build()
            .header("Authorization", format!("Bearer {token}"))
            .call()
            .with_context(|| format!("GET /api{path}"))?;
        self.accept(res)
    }

    /// `None` when the server no longer accepts the token; it's also dropped
    /// from the keyring so the next launch doesn't retry it.
    fn accept<T: DeserializeOwned>(&self, res: http::Response<ureq::Body>) -> Result<Option<T>> {
        let body = read_api(res)?;
        if body.is_none() {
            forget(&self.keyring_entry()?)?;
        }
        Ok(body)
    }
}

/// The server puts a user-facing reason in error bodies, e.g. "Voting is
/// closed for this week".
#[derive(Deserialize)]
struct ErrorBody {
    detail: String,
}

fn read_api<T: DeserializeOwned>(mut res: http::Response<ureq::Body>) -> Result<Option<T>> {
    let status = res.status();
    if status == http::StatusCode::UNAUTHORIZED {
        return Ok(None);
    }
    if !status.is_success() {
        let detail = res.body_mut().read_json::<ErrorBody>().map(|b| b.detail);
        bail!(detail.unwrap_or_else(|_| format!("Request failed ({status})")));
    }
    res.body_mut()
        .read_json()
        .map(Some)
        .context("reading response")
}

fn forget(entry: &keyring::Entry) -> Result<()> {
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e).context("deleting saved session"),
    }
}

fn random_b64(n: usize) -> String {
    let mut bytes = vec![0u8; n];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Must match the server's S256 check in `accounts::pkce_challenge`.
fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::pkce_challenge;

    #[test]
    fn pkce_challenge_matches_rfc_7636_vector() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
}
