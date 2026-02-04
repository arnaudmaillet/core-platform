// crates/profile/src/domain/value_objects.rs

use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::Url;
use shared_kernel::errors::{DomainError, Result};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SocialLinks {
    #[serde(skip_serializing_if = "Option::is_none")]
    website: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    linkedin: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    github: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    x: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instagram: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    facebook: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tiktok: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    youtube: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    twitch: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    discord: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    onlyfans: Option<Url>,

    #[serde(flatten)]
    others: HashMap<String, Url>,
}

impl SocialLinks {

    pub fn new() -> Self {
        Self::default()
    }

    // --- Getters ---

    pub fn website(&self) -> Option<&Url> { self.website.as_ref() }
    pub fn linkedin(&self) -> Option<&Url> { self.linkedin.as_ref() }
    pub fn github(&self) -> Option<&Url> { self.github.as_ref() }
    pub fn x(&self) -> Option<&Url> { self.x.as_ref() }
    pub fn instagram(&self) -> Option<&Url> { self.instagram.as_ref() }
    pub fn facebook(&self) -> Option<&Url> { self.facebook.as_ref() }
    pub fn tiktok(&self) -> Option<&Url> { self.tiktok.as_ref() }
    pub fn youtube(&self) -> Option<&Url> { self.youtube.as_ref() }
    pub fn twitch(&self) -> Option<&Url> { self.twitch.as_ref() }
    pub fn discord(&self) -> Option<&Url> { self.discord.as_ref() }
    pub fn onlyfans(&self) -> Option<&Url> { self.onlyfans.as_ref() }
    pub fn others(&self) -> &HashMap<String, Url> { &self.others }


    // --- Fluent Setters (Immutable update pattern) ---

    pub fn with_website(mut self, url: Option<Url>) -> Self { self.website = url; self }
    pub fn with_linkedin(mut self, url: Option<Url>) -> Self { self.linkedin = url; self }
    pub fn with_github(mut self, url: Option<Url>) -> Self { self.github = url; self }
    pub fn with_x(mut self, url: Option<Url>) -> Self { self.x = url; self }
    pub fn with_instagram(mut self, url: Option<Url>) -> Self { self.instagram = url; self }
    pub fn with_facebook(mut self, url: Option<Url>) -> Self { self.facebook = url; self }
    pub fn with_tiktok(mut self, url: Option<Url>) -> Self { self.tiktok = url; self }
    pub fn with_youtube(mut self, url: Option<Url>) -> Self { self.youtube = url; self }
    pub fn with_twitch(mut self, url: Option<Url>) -> Self { self.twitch = url; self }
    pub fn with_discord(mut self, url: Option<Url>) -> Self { self.discord = url; self }
    pub fn with_onlyfans(mut self, url: Option<Url>) -> Self { self.onlyfans = url; self }
    pub fn with_other(mut self, key: String, url: Url) -> Self {
        self.others.insert(key, url);
        self
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

    pub fn try_build(mut self) -> Result<Option<Self>> {
        // 1. Validation : Règle métier sur le nombre de liens personnalisés
        if self.others.len() > 10 {
            return Err(DomainError::Validation {
                field: "social_links",
                reason: "Too many custom links (max 10)".into(),
            });
        }

        // 2. Nettoyage : On retire les clés vides ou composées d'espaces dans 'others'
        self.others.retain(|key, _| !key.trim().is_empty());

        // 3. Vérification de la vacuité totale
        // On vérifie si TOUS les champs sont à None et si la HashMap est vide
        let is_empty = self.website.is_none()
            && self.linkedin.is_none()
            && self.github.is_none()
            && self.x.is_none()
            && self.instagram.is_none()
            && self.facebook.is_none()
            && self.tiktok.is_none()
            && self.youtube.is_none()
            && self.twitch.is_none()
            && self.discord.is_none()
            && self.onlyfans.is_none()
            && self.others.is_empty();

        if is_empty {
            // L'objet n'a aucun intérêt technique, on retourne None
            Ok(None)
        } else {
            // L'objet est valide et contient au moins un lien
            Ok(Some(self))
        }
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
