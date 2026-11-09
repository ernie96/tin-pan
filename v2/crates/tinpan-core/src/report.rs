//! Report a problem and Suggest a feature, as Tin Pan 1 sends them: to Tin Pan's report relay, which files each
//! one privately for Tin Pan's author. Nobody needs an account, and the log goes with your user name taken out.

use std::time::Duration;

use reqwest::blocking::Client;
use serde::Serialize;

use crate::i18n::{t, tf};

/// Where reports go, Tin Pan 1's relay.
pub const RELAY_URL: &str = "https://tinpan-reports.ernie96.workers.dev/report";
pub const MAX_DESCRIPTION: usize = 5000;
pub const MAX_CONTACT: usize = 200;
pub const MAX_LOG: usize = 200_000;

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// "bug" or "feature".
    pub kind: &'static str,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,
    pub version: String,
    pub language: String,
    /// The system, under Tin Pan 1's name for it.
    pub windows: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log: Option<String>,
}

fn clip(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// Paths hold the user name, often a real name: C:\Users\Ana\... becomes C:\Users\<user>\..., and so do
/// /home/ana/ and /Users/ana/. The name on its own is taken out as a whole word only, so a short name
/// doesn't cut into other words.
pub fn anonymize(log: &str, user_name: Option<&str>) -> String {
    let mut out = String::with_capacity(log.len());
    let mut i = 0;
    while i < log.len() {
        let marker = ["\\users\\", "/users/", "/home/"].iter().find(|m| log.as_bytes()[i..].get(..m.len()).is_some_and(|b| b.eq_ignore_ascii_case(m.as_bytes())));
        match marker {
            Some(m) => {
                out.push_str(&log[i..i + m.len()]);
                i += m.len();
                let end = log[i..].find(['\\', '/', ':', '*', '?', '"', '<', '>', '|', '\r', '\n']).map_or(log.len(), |e| i + e);
                if end > i {
                    out.push_str("<user>");
                }
                i = end;
            }
            None => {
                let c = log[i..].chars().next().expect("inside the text");
                out.push(c);
                i += c.len_utf8();
            }
        }
    }
    let Some(name) = user_name.map(str::trim).filter(|n| n.chars().count() >= 3) else { return out };
    // Whole words only.
    let (name_lower, text_lower) = (name.to_lowercase(), out.to_lowercase());
    if name_lower.len() != name.len() || text_lower.len() != out.len() {
        return out;
    }
    let mut result = String::with_capacity(out.len());
    let mut at = 0;
    while let Some(found) = text_lower[at..].find(&name_lower) {
        let start = at + found;
        let end = start + name.len();
        let before = out[..start].chars().next_back().is_some_and(char::is_alphanumeric);
        let after = out[end..].chars().next().is_some_and(char::is_alphanumeric);
        result.push_str(&out[at..start]);
        result.push_str(if before || after { &out[start..end] } else { "<user>" });
        at = end;
    }
    result.push_str(&out[at..]);
    result
}

/// The end of `text`, at most `max` bytes, starting on a whole character.
fn last_bytes(text: &str, max: usize) -> &str {
    let mut start = text.len().saturating_sub(max);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

/// Sends the report; its reference number, or what to say.
pub fn send(http: &Client, report: &Report) -> Result<i64, String> {
    if report.description.trim().is_empty() {
        return Err(t("Please describe it first."));
    }
    let report = Report {
        description: clip(report.description.trim(), MAX_DESCRIPTION),
        contact: report.contact.as_deref().map(str::trim).filter(|c| !c.is_empty()).map(|c| clip(c, MAX_CONTACT)),
        log: report.log.as_ref().map(|l| last_bytes(l, MAX_LOG).to_string()),
        ..report.clone()
    };
    let response = http
        .post(RELAY_URL)
        .header("X-TinPan-Report", "1")
        .timeout(Duration::from_secs(60))
        .json(&report)
        .send()
        .map_err(|e| {
            log::warn!("Couldn't send a report: {e}");
            t("Couldn't reach the report service. Check your internet connection and try again.")
        })?;
    let status = response.status().as_u16();
    let body = response.text().unwrap_or_default();
    let number = serde_json::from_str::<serde_json::Value>(&body).ok().and_then(|v| v.get("number").and_then(serde_json::Value::as_i64));
    match number {
        Some(n) if (200..300).contains(&status) => {
            log::info!("Report sent as number {n}");
            Ok(n)
        }
        _ => {
            log::warn!("The report relay answered {status}: {}", clip(&body, 300));
            Err(if status == 429 { t("Too many reports were sent from this connection just now. Wait a minute and try again.") } else { tf("The report service answered with an error ({0}).", &[&status]) })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_names_leave_the_log() {
        let log = "Opened C:\\Users\\Ana Maria\\Music\\a.mp3 and /home/ana/x; Ana said hi; Banana stays";
        assert_eq!(anonymize(log, Some("Ana")), "Opened C:\\Users\\<user>\\Music\\a.mp3 and /home/<user>/x; <user> said hi; Banana stays");
        assert_eq!(anonymize("Al was here", Some("Al")), "Al was here");
        // Characters whose lowercase is longer must not shift or split anything.
        assert_eq!(anonymize("İİİ C:\\Users\\Ana\\x and İ", None), "İİİ C:\\Users\\<user>\\x and İ");
        assert_eq!(last_bytes("aé", 1), "");
        assert_eq!(last_bytes("aé", 2), "é");
    }
}
