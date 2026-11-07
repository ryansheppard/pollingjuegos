//! Everything here blocks (network, loopback listener, OS keyring), so the UI
//! only calls it from a worker thread.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use api_types::{DesktopTokenRequest, DesktopTokenResponse, Me, QuarterbackWithStats};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::Rng;
use sha2::{Digest, Sha256};

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
        match self.me(&token)? {
            Some(user_id) => Ok(Some(Session { token, user_id })),
            None => {
                forget(&entry)?;
                Ok(None)
            }
        }
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
        let res = self
            .agent
            .get(format!("{}/api/me", self.server))
            .header("Authorization", format!("Bearer {token}"))
            .call();
        match res {
            Ok(mut res) => Ok(Some(res.body_mut().read_json::<Me>()?.id)),
            Err(ureq::Error::StatusCode(401)) => Ok(None),
            Err(e) => Err(e).context("checking session"),
        }
    }

    /// `None` when the server no longer accepts the token; it's also dropped
    /// from the keyring so the next launch doesn't retry it.
    pub fn quarterbacks(&self, token: &str) -> Result<Option<Vec<QuarterbackWithStats>>> {
        let mut all = Vec::new();
        loop {
            let res = self
                .agent
                .get(format!("{}/api/quarterbacks", self.server))
                .query("limit", QUARTERBACK_PAGE.to_string())
                .query("offset", all.len().to_string())
                .header("Authorization", format!("Bearer {token}"))
                .call();
            let page = match res {
                Ok(mut res) => res
                    .body_mut()
                    .read_json::<Vec<QuarterbackWithStats>>()
                    .context("reading quarterbacks")?,
                Err(ureq::Error::StatusCode(401)) => {
                    forget(&self.keyring_entry()?)?;
                    return Ok(None);
                }
                Err(e) => return Err(e).context("loading quarterbacks"),
            };
            let done = page.len() < QUARTERBACK_PAGE;
            all.extend(page);
            if done {
                return Ok(Some(all));
            }
        }
    }
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
