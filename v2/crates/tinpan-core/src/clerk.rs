//! Suno signs in through Clerk. The browser's "__client" cookie identifies the device; from it Tin Pan
//! reads the active session, then mints short-lived (about 60 second) tokens for Suno's API.
//! Blocking calls: they run on Tin Pan's network thread, never the window's.

use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, COOKIE, SET_COOKIE};
use serde_json::Value;

use crate::cookies::{SunoCookies, jwt_expiry};
use crate::error::{Error, Result};
use crate::i18n::{t, tf};

/// auth.suno.com is what suno.com uses now; clerk.suno.com is the older name.
const CLERK_BASES: [&str; 2] = ["https://auth.suno.com", "https://clerk.suno.com"];
const CLERK_QUERY: &str = "__clerk_api_version=2025-11-10&_clerk_js_version=5.117.0";
const REFRESH_MARGIN: chrono::TimeDelta = chrono::TimeDelta::seconds(15);

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

/// The HTTP client every Suno call shares: one connection pool, a browser's user agent, no cookie jar
/// of its own (Tin Pan sends its cookies itself).
pub fn http_client() -> Client {
    // ring as rustls' crypto, picked once for the whole process.
    let _ = rustls::crypto::ring::default_provider().install_default();
    Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(30))
        .build()
        .expect("the HTTP client builds")
}

/// The same, for asking where a link leads without going there.
pub fn no_redirects_client() -> Client {
    let _ = rustls::crypto::ring::default_provider().install_default();
    Client::builder().user_agent(USER_AGENT).timeout(Duration::from_secs(30)).redirect(reqwest::redirect::Policy::none()).build().expect("the HTTP client builds")
}

pub struct ClerkSession {
    http: Client,
    pub cookies: SunoCookies,
    clerk_base: Option<&'static str>,
    session_id: Option<String>,
    jwt: Option<(String, DateTime<Utc>)>,
    /// The signed-in account's username, or full name.
    pub user_name: Option<String>,
    /// Set when Clerk rotated the cookies, so they can be saved again.
    pub cookies_changed: bool,
}

impl ClerkSession {
    pub fn new(http: Client, cookies: SunoCookies) -> Self {
        Self { http, cookies, clerk_base: None, session_id: None, jwt: None, user_name: None, cookies_changed: false }
    }

    pub fn is_ready(&self) -> bool {
        self.session_id.is_some()
    }

    /// Finds the active session. SignInRequired when there's none.
    pub fn initialize(&mut self) -> Result<()> {
        if !self.cookies.has_client() {
            return Err(Error::SignInRequired(t("Not signed in.")));
        }
        let mut last = None;
        for base in CLERK_BASES {
            let response = match self.send(reqwest::Method::GET, &format!("{base}/v1/client?{CLERK_QUERY}"), None) {
                Ok(r) => r,
                Err(e) => {
                    log::error!("Clerk client lookup on {base} failed: {e}");
                    last = Some(e);
                    continue;
                }
            };
            let status = response.status();
            if status.as_u16() == 404 {
                continue;
            }
            if !status.is_success() {
                log::warn!("Clerk client lookup on {base} returned {status}");
                last = Some(Error::api_status(tf("Suno sign-in check failed ({0}).", &[&status.as_u16()]), status.as_u16()));
                continue;
            }
            let Ok(root) = response.json::<Value>() else {
                last = Some(Error::api(t("Suno's sign-in service sent a reply this app doesn't understand.")));
                continue;
            };
            // Clerk answers {"response": null} when this browser has no signed-in client left.
            let client = root.get("response").unwrap_or(&root);
            let Some(sid) = client.get("last_active_session_id").and_then(Value::as_str).filter(|s| !s.is_empty()) else {
                return Err(Error::SignInRequired(t("Not signed in to Suno.")));
            };
            self.user_name = user_name(client, sid);
            self.session_id = Some(sid.to_string());
            self.clerk_base = Some(base);
            log::info!("Clerk session found via {base}");
            return Ok(());
        }
        Err(last.unwrap_or_else(|| Error::api(t("Suno sign-in service didn't answer."))))
    }

    /// A token good for at least a few more seconds.
    pub fn token(&mut self, force_refresh: bool) -> Result<String> {
        if !force_refresh
            && let Some((jwt, expires)) = &self.jwt
            && Utc::now() < *expires - REFRESH_MARGIN
        {
            return Ok(jwt.clone());
        }
        if self.session_id.is_none() {
            self.initialize()?;
        }
        let (base, sid) = (self.clerk_base.unwrap_or(CLERK_BASES[0]), self.session_id.clone().unwrap_or_default());
        let url = format!("{base}/v1/client/sessions/{sid}/tokens?{CLERK_QUERY}");
        let response = self.send(reqwest::Method::POST, &url, Some("{}"))?;
        let status = response.status().as_u16();
        if matches!(status, 401 | 403 | 404) {
            self.session_id = None;
            return Err(Error::SignInRequired(t("Not signed in to Suno.")));
        }
        if !response.status().is_success() {
            return Err(Error::api_status(tf("Couldn't refresh the Suno sign-in ({0}).", &[&status]), status));
        }
        let body: Value = response.json().map_err(|_| Error::api(t("Suno's sign-in service sent a reply this app doesn't understand.")))?;
        let jwt = body.get("jwt").and_then(Value::as_str).filter(|j| !j.is_empty()).ok_or_else(|| Error::api(t("Suno sign-in returned no token.")))?;
        let expires = jwt_expiry(jwt).unwrap_or_else(|| Utc::now() + chrono::TimeDelta::seconds(50));
        self.jwt = Some((jwt.to_string(), expires));
        Ok(jwt.to_string())
    }

    fn send(&mut self, method: reqwest::Method, url: &str, body: Option<&str>) -> Result<reqwest::blocking::Response> {
        // Clerk's native flow takes the raw __client value, not "Bearer ...". No Origin or Referer:
        // with them Clerk treats this as a browser call, and sign-in stops working.
        let mut request = self
            .http
            .request(method, url)
            .header(AUTHORIZATION, self.cookies.client().unwrap_or_default())
            .header(COOKIE, self.cookies.to_header());
        if let Some(body) = body {
            request = request.header(CONTENT_TYPE, "application/json").body(body.to_string());
        }
        let response = request.send().map_err(|_| Error::api(t("Can't reach Suno. Check your internet connection.")))?;
        let before = self.cookies.client().map(str::to_string);
        for value in response.headers().get_all(SET_COOKIE) {
            if let Ok(text) = value.to_str() {
                self.cookies.merge_set_cookie(text);
            }
        }
        if self.cookies.client().map(str::to_string) != before {
            self.cookies_changed = true;
        }
        Ok(response)
    }
}

fn user_name(client: &Value, sid: &str) -> Option<String> {
    let session = client.get("sessions")?.as_array()?.iter().find(|s| s.get("id").and_then(Value::as_str) == Some(sid))?;
    let user = session.get("user")?;
    let text = |name: &str| user.get(name).and_then(Value::as_str).unwrap_or("").trim().to_string();
    let username = text("username");
    if !username.is_empty() {
        return Some(username);
    }
    let full = format!("{} {}", text("first_name"), text("last_name")).trim().to_string();
    (!full.is_empty()).then_some(full)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_username_or_the_full_name() {
        let client = serde_json::json!({"sessions": [{"id": "s1", "user": {"username": null, "first_name": "Ana", "last_name": "Pop"}}]});
        assert_eq!(user_name(&client, "s1").as_deref(), Some("Ana Pop"));
        assert_eq!(user_name(&client, "other"), None);
    }

    #[test]
    fn without_a_client_cookie_it_asks_to_sign_in() {
        let mut session = ClerkSession::new(http_client(), SunoCookies::parse(Some("__session=x")));
        assert!(matches!(session.initialize(), Err(Error::SignInRequired(_))));
    }
}
