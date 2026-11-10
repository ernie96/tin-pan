//! The account's plan (GET /api/billing/info/): credits, downloads, free allowances and what the plan
//! includes, said the way Tin Pan 1 says them (F2 and Shift+F2).

use chrono::DateTime;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::i18n::{plural, t, tf};

fn int<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f.round() as i64)),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    })
}

fn models<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<crate::create::SunoModel>, D::Error> {
    Ok(crate::create::read_models(&Value::deserialize(d)?))
}

fn flag<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(matches!(Value::deserialize(d)?, Value::Bool(true)))
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct BillingInfo {
    #[serde(deserialize_with = "int")]
    pub total_credits_left: Option<i64>,
    /// Credits bought on top of the plan.
    #[serde(rename = "credits", deserialize_with = "int")]
    pub extra_credits: Option<i64>,
    #[serde(deserialize_with = "int")]
    pub monthly_limit: Option<i64>,
    #[serde(deserialize_with = "int")]
    pub monthly_usage: Option<i64>,
    pub renews_on: Option<String>,
    #[serde(deserialize_with = "flag")]
    pub is_active: bool,
    pub plan: Option<Plan>,
    pub accessible_features: Vec<Feature>,
    pub download_usage: Option<DownloadUsage>,
    #[serde(rename = "free_cover_clips_remaining", deserialize_with = "int")]
    pub free_covers_left: Option<i64>,
    #[serde(rename = "free_remasters_remaining", deserialize_with = "int")]
    pub free_remasters_left: Option<i64>,
    /// Every model Suno lists; `create::usable` picks the ones this plan may use.
    #[serde(deserialize_with = "models")]
    pub models: Vec<crate::create::SunoModel>,
    #[serde(rename = "remaster_model_types", deserialize_with = "models")]
    pub remaster_models: Vec<crate::create::SunoModel>,
    /// Shortest and longest audio upload this account may make, in seconds.
    pub audio_upload_limits: Option<UploadLimits>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(default)]
pub struct UploadLimits {
    min: Option<f64>,
    max: Option<f64>,
}

impl UploadLimits {
    /// Values that can't be seconds (zero, negative, or over 10 hours) count as unknown, as in Tin Pan 1.
    pub fn min_seconds(&self) -> Option<f64> {
        self.min.filter(|m| *m > 0.0 && *m < 36_000.0)
    }

    pub fn max_seconds(&self) -> Option<f64> {
        self.max.filter(|m| *m > 0.0 && *m < 36_000.0)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Plan {
    pub name: Option<String>,
    pub plan_key: Option<String>,
    #[serde(rename = "usage_plan_features")]
    pub features: Vec<Feature>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Feature {
    pub name: String,
}

/// Since September 2026: free 7 in total, Pro 20 a month, Premier 60 a month.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DownloadUsage {
    #[serde(rename = "current_period_downloads_limit", deserialize_with = "int")]
    pub limit: Option<i64>,
    #[serde(rename = "current_period_downloads_used", deserialize_with = "int")]
    pub used: Option<i64>,
    #[serde(rename = "additional_download_remaining", deserialize_with = "int")]
    pub extra: Option<i64>,
}

impl DownloadUsage {
    /// Downloads left this period, bought ones included; None when Suno doesn't say.
    pub fn remaining(&self) -> Option<i64> {
        match self.limit {
            Some(limit) => Some((limit - self.used.unwrap_or(0)).max(0) + self.extra.unwrap_or(0)),
            None => self.extra,
        }
    }
}

impl BillingInfo {
    pub fn read(json: &str) -> crate::Result<Self> {
        serde_json::from_str(json).map_err(|_| crate::Error::api(t("Suno sent a reply this app doesn't understand.")))
    }

    /// True for paid plans, false for the free plan, None when the reply doesn't say. WAV needs a paid plan.
    /// A reply that names no plan is the free plan when there's no active subscription either.
    pub fn is_paid(&self) -> Option<bool> {
        let key = self.plan.as_ref().and_then(|p| p.plan_key.as_deref()).unwrap_or("").trim().to_lowercase();
        let name = self.plan.as_ref().and_then(|p| p.name.as_deref()).unwrap_or("").trim().to_lowercase();
        if key.is_empty() && name.is_empty() {
            return if self.is_active { None } else { Some(false) };
        }
        Some(!(key.contains("free") || name.contains("free") || key == "basic" || name == "basic"))
    }

    /// Suno Studio accounts download through Studio's own route.
    pub fn has_studio(&self) -> bool {
        self.accessible_features.iter().any(|f| f.name == "studio")
    }

    /// F2: total credits, then where they come from.
    pub fn credits_text(&self) -> String {
        let total = self.total_credits_left.unwrap_or(0);
        let mut text = plural(total, "{0:N0} credit left.", "{0:N0} credits left.");
        match (self.monthly_limit.filter(|l| *l > 0), self.monthly_usage) {
            (Some(limit), Some(used)) => {
                let left = (limit - used).max(0);
                text += " ";
                text += &match self.extra_credits.filter(|e| *e > 0) {
                    Some(extra) => tf("{0} of this month's {1}, plus {2} extra.", &[&n(left), &n(limit), &n(extra)]),
                    None => tf("{0} of this month's {1}.", &[&n(left), &n(limit)]),
                };
            }
            _ => {
                if let Some(extra) = self.extra_credits.filter(|e| *e > 0) {
                    text += " ";
                    text += &tf("Including {0} extra.", &[&n(extra)]);
                }
            }
        }
        text
    }

    /// Shift+F2: plan, renewal, credits, downloads and the free plan's allowances.
    pub fn subscription_text(&self, style_personalization: Option<bool>) -> String {
        let state = if self.is_active { t("active") } else { t("not active") };
        // The free plan isn't a subscription, so "not active" says nothing about it.
        let free = self.is_paid() == Some(false) && !self.is_active;
        let mut parts = vec![match (self.renewal(), free) {
            (_, true) => format!("{}.", self.plan_name()),
            (Some(renews), false) => tf("{0}, {1}, renews {2}.", &[&self.plan_name(), &state, &renews]),
            (None, false) => tf("{0}, {1}.", &[&self.plan_name(), &state]),
        }];
        parts.push(self.credits_text());
        if let Some(usage) = &self.download_usage
            && let Some(left) = usage.remaining()
        {
            let count = plural(left, "{0:N0} download left", "{0:N0} downloads left");
            parts.push(match (usage.limit, usage.extra.filter(|e| *e > 0)) {
                (None, _) => count + ".",
                (Some(limit), Some(extra)) => tf("{0}: {1} a month, {2} used, plus {3} extra.", &[&count, &n(limit), &n(usage.used.unwrap_or(0)), &n(extra)]),
                (Some(limit), None) => tf("{0}: {1} a month, {2} used.", &[&count, &n(limit), &n(usage.used.unwrap_or(0))]),
            });
        }
        if self.is_paid() == Some(false) {
            parts.push(t("WAV downloads need Pro or Premier."));
            if let Some(covers) = self.free_covers_left {
                parts.push(plural(covers, "{0} free cover left.", "{0} free covers left."));
            }
            parts.push(match self.free_remasters_left.filter(|r| *r > 0) {
                Some(remasters) => plural(remasters, "{0} free remaster left.", "{0} free remasters left."),
                None => t("Remasters need Pro or Premier."),
            });
        }
        match style_personalization {
            Some(true) => parts.push(t("Style ideas and boosts are shaped to your taste.")),
            Some(false) => parts.push(t("Style ideas and boosts aren't shaped to your taste.")),
            None => {}
        }
        parts.join(" ")
    }

    /// "Your plan includes: covers, personas, stems." from the plan's and the account's features.
    pub fn features_text(&self) -> String {
        let mut names: Vec<String> = Vec::new();
        let plan = self.plan.as_ref().map(|p| p.features.as_slice()).unwrap_or_default();
        for raw in plan.iter().chain(&self.accessible_features).map(|f| f.name.trim()).filter(|n| !n.is_empty()) {
            let name = feature_name(raw);
            if !names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
                names.push(name);
            }
        }
        if names.is_empty() { String::new() } else { tf("Your plan includes: {0}.", &[&names.join(", ")]) }
    }

    pub fn plan_name(&self) -> String {
        let name = self.plan.as_ref().and_then(|p| p.name.as_deref()).map(str::trim).unwrap_or("");
        if name.is_empty() {
            return if self.is_paid() == Some(false) { t("Free plan") } else { t("Plan unknown") };
        }
        let name = if name.to_lowercase().ends_with(" plan") { name[..name.len() - 5].trim_end() } else { name };
        tf("{0} plan", &[&name])
    }

    /// The renewal date in the chosen language ("November 9, 2026"), or Suno's own text if it isn't a date.
    pub fn renewal(&self) -> Option<String> {
        let raw = self.renews_on.as_deref().map(str::trim).filter(|r| !r.is_empty())?;
        let date = DateTime::parse_from_rfc3339(raw)
            .map(|d| d.with_timezone(&chrono::Local).date_naive())
            .or_else(|_| chrono::NaiveDate::parse_from_str(raw.get(..10).unwrap_or(raw), "%Y-%m-%d"));
        Some(match date {
            Ok(date) => crate::i18n::long_date(date),
            Err(_) => raw.to_string(),
        })
    }
}

fn n(value: i64) -> String {
    crate::i18n::group_digits(value)
}

/// Suno's feature keys in words, as Tin Pan 1 says them; others with their underscores made spaces.
fn feature_name(raw: &str) -> String {
    let key = raw.trim().replace(' ', "_").to_lowercase();
    let name = match key.as_str() {
        "v4" => "v4 model",
        "auk" => "v4.5 models",
        "cover" => "covers",
        "edit_mode" => "editing songs",
        "persona" => "personas",
        "can_buy_credit_top_ups" => "buying extra credits",
        "commercial_rights" => "commercial use",
        "get_stems" => "stems",
        "generate_song_image" => "song cover images",
        "negative_tags" => "styles to exclude",
        "remaster" => "remasters",
        "generate_song_video" => "song videos",
        "long_uploads" => "long uploads",
        "convert_audio" => "WAV conversion",
        "create_control_sliders" => "weirdness and style sliders",
        "playlist_condition" => "playlist inspiration",
        "tag_upsample" => "style boost",
        "custom_models" => "custom models",
        "max_mode" => "max mode",
        "studio" => return "Suno Studio".into(),
        _ => return raw.trim().replace('_', " "),
    };
    t(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FREE: &str = r#"{"total_credits_left": 1250, "credits": 200, "monthly_limit": 50, "monthly_usage": 20, "renews_on": "2026-11-09T00:00:00Z",
        "is_active": true, "plan": {"name": "Free plan", "plan_key": "free"},
        "download_usage": {"current_period_downloads_limit": 7, "current_period_downloads_used": 2},
        "free_cover_clips_remaining": 3, "free_remasters_remaining": 0}"#;

    #[test]
    fn says_the_plan_as_tin_pan_1_does() {
        let info = BillingInfo::read(FREE).unwrap();
        assert_eq!(info.is_paid(), Some(false));
        assert_eq!(info.credits_text(), "1,250 credits left. 30 of this month's 50, plus 200 extra.");
        let text = info.subscription_text(Some(true));
        assert!(text.starts_with("Free plan, active, renews November "), "{text}");
        assert!(text.contains("5 downloads left: 7 a month, 2 used."), "{text}");
        assert!(text.contains("3 free covers left. Remasters need Pro or Premier."), "{text}");
        assert!(text.ends_with("Style ideas and boosts are shaped to your taste."));
    }

    #[test]
    fn a_paid_plan_and_odd_values() {
        let info = BillingInfo::read(r#"{"total_credits_left": "12", "plan": {"name": "Pro"}, "accessible_features": [{"name": "studio"}]}"#).unwrap();
        assert_eq!((info.is_paid(), info.has_studio(), info.total_credits_left), (Some(true), true, Some(12)));
        assert_eq!(BillingInfo::read(r#"{"is_active": true}"#).unwrap().is_paid(), None);
        // A free account: no plan named, no active subscription.
        let free = BillingInfo::read(r#"{"is_active": false, "plan": null, "total_credits_left": 910, "credits": 810, "monthly_limit": 100, "monthly_usage": 0}"#).unwrap();
        assert_eq!((free.is_paid(), free.plan_name()), (Some(false), "Free plan".to_string()));
        assert!(free.subscription_text(None).starts_with("Free plan. 910 credits left."));
    }
}
