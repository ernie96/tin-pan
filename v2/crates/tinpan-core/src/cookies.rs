//! The cookie jar for suno.com and its Clerk sign-in domain, kept as name/value pairs. The long-lived
//! "__client" cookie is what lets Tin Pan mint fresh API tokens.

use base64::Engine;
use chrono::{DateTime, Utc};

const ATTRIBUTES: [&str; 8] = ["expires", "max-age", "domain", "path", "secure", "httponly", "samesite", "partitioned"];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SunoCookies {
    // In the order first seen, so the header keeps a stable order.
    values: Vec<(String, String)>,
}

impl SunoCookies {
    /// Parses "a=1; b=2" or Set-Cookie style text, skipping cookie attributes.
    pub fn parse(header: Option<&str>) -> Self {
        let mut jar = Self::default();
        jar.merge(header);
        jar
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }

    pub fn client(&self) -> Option<&str> {
        self.get("__client")
    }

    /// Suno's analytics id doubles as the Device-Id header in the official clients.
    pub fn anonymous_id(&self) -> Option<&str> {
        self.get("ajs_anonymous_id")
    }

    pub fn has_client(&self) -> bool {
        self.client().is_some_and(|c| !c.is_empty())
    }

    pub fn set(&mut self, name: &str, value: &str) {
        let name = name.trim();
        if name.is_empty() || ATTRIBUTES.iter().any(|a| a.eq_ignore_ascii_case(name)) {
            return;
        }
        let value = value.trim().to_string();
        match self.values.iter_mut().find(|(n, _)| n == name) {
            Some(entry) => entry.1 = value,
            None => self.values.push((name.to_string(), value)),
        }
    }

    pub fn merge(&mut self, header: Option<&str>) {
        let Some(header) = header else { return };
        for part in header.split(';') {
            if let Some((name, value)) = part.split_once('=')
                && !name.trim().is_empty()
            {
                self.set(name, value);
            }
        }
    }

    /// Merges the name=value pair at the start of a Set-Cookie header.
    pub fn merge_set_cookie(&mut self, set_cookie: &str) {
        let first = set_cookie.split(';').next().unwrap_or("");
        if let Some((name, value)) = first.split_once('=')
            && !name.trim().is_empty()
        {
            self.set(name, value);
        }
    }

    pub fn to_header(&self) -> String {
        self.values.iter().map(|(n, v)| format!("{n}={v}")).collect::<Vec<_>>().join("; ")
    }
}

/// Reads a JWT's "exp" claim without checking its signature. None if it can't be read.
pub fn jwt_expiry(token: &str) -> Option<DateTime<Utc>> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(payload.trim_end_matches('=')))
        .ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    DateTime::from_timestamp(value.get("exp")?.as_i64()?, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_keeps_values_and_skips_cookie_attributes() {
        let jar = SunoCookies::parse(Some("__client=abc.def; Path=/; HttpOnly; ajs_anonymous_id=dev-1; Secure; __session=eyJ=x"));
        assert!(jar.has_client());
        assert_eq!(jar.client(), Some("abc.def"));
        assert_eq!(jar.anonymous_id(), Some("dev-1"));
        assert_eq!(jar.get("__session"), Some("eyJ=x"));
        assert_eq!(jar.get("Path"), None);
    }

    #[test]
    fn merge_set_cookie_replaces_the_rotated_client_cookie() {
        let mut jar = SunoCookies::parse(Some("__client=old"));
        jar.merge_set_cookie("__client=new; Domain=auth.suno.com; Expires=Wed, 21 Oct 2027 07:28:00 GMT; HttpOnly");
        assert_eq!(jar.client(), Some("new"));
        assert_eq!(jar.to_header(), "__client=new");
    }

    #[test]
    fn no_client_cookie_means_not_signed_in() {
        assert!(!SunoCookies::parse(Some("__session=x; ajs_anonymous_id=y")).has_client());
        assert!(!SunoCookies::parse(None).has_client());
    }

    fn token(payload: &str) -> String {
        let b64 = |s: &str| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(s);
        format!("{}.{}.sig", b64(r#"{"alg":"none"}"#), b64(payload))
    }

    #[test]
    fn jwt_expiry_reads_exp_and_refuses_garbage() {
        assert_eq!(jwt_expiry(&token(r#"{"exp":1800000000,"sid":"s"}"#)), DateTime::from_timestamp(1_800_000_000, 0));
        for bad in ["", "not-a-jwt", "a.!!!.c"] {
            assert_eq!(jwt_expiry(bad), None);
        }
        assert_eq!(jwt_expiry(&token(r#"{"exp":99999999999999}"#)), None);
    }
}
