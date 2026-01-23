// crates/profile/src/domain/value_objects.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use shared_kernel::domain::value_objects::Url;
use shared_kernel::errors::{DomainError, Result};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SocialLinks {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linkedin: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instagram: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facebook: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tiktok: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub youtube: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twitch: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discord: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onlyfans: Option<Url>,

    #[serde(flatten)]
    pub others: HashMap<String, Url>,
}

impl SocialLinks {
    /// Nettoyage et validation pour l'Hyperscale
    pub fn simplify(mut self) -> Option<Self> {
        self.others.retain(|key, _| !key.trim().is_empty());

        let is_empty = self.website.is_none() &&
            self.linkedin.is_none() &&
            self.github.is_none() &&
            self.x.is_none() &&
            self.instagram.is_none() &&
            self.facebook.is_none() &&
            self.tiktok.is_none() &&
            self.youtube.is_none() &&
            self.twitch.is_none() &&
            self.discord.is_none() &&
            self.onlyfans.is_none() &&
            self.others.is_empty();

        if is_empty { None } else { Some(self) }
    }

    pub fn validate(&self) -> Result<()> {
        if self.others.len() > 10 {
            return Err(DomainError::Validation {
                field: "social_links",
                reason: "Too many custom links (max 10)".into(),
            });
        }
        Ok(())
    }
}
// --- CONVERSIONS---

impl From<serde_json::Value> for SocialLinks {
    fn from(value: serde_json::Value) -> Self {
        serde_json::from_value(value).unwrap_or_default()
    }
}

impl From<SocialLinks> for serde_json::Value {
    fn from(links: SocialLinks) -> Self {
        serde_json::to_value(links).unwrap_or(serde_json::json!({}))
    }
}