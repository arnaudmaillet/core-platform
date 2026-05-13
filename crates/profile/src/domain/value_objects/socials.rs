// crates/profile/src/domain/value_objects.rs

use serde::{Deserialize, Serialize};
use shared_kernel::{
    core::{Error, Result},
    types::Url,
};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Socials {
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

impl Socials {
    pub fn builder() -> Self {
        Self::default()
    }

    pub fn build(self) -> Self {
        self
    }

    // --- Getters ---

    pub fn website(&self) -> Option<&Url> {
        self.website.as_ref()
    }
    pub fn linkedin(&self) -> Option<&Url> {
        self.linkedin.as_ref()
    }
    pub fn github(&self) -> Option<&Url> {
        self.github.as_ref()
    }
    pub fn x(&self) -> Option<&Url> {
        self.x.as_ref()
    }
    pub fn instagram(&self) -> Option<&Url> {
        self.instagram.as_ref()
    }
    pub fn facebook(&self) -> Option<&Url> {
        self.facebook.as_ref()
    }
    pub fn tiktok(&self) -> Option<&Url> {
        self.tiktok.as_ref()
    }
    pub fn youtube(&self) -> Option<&Url> {
        self.youtube.as_ref()
    }
    pub fn twitch(&self) -> Option<&Url> {
        self.twitch.as_ref()
    }
    pub fn discord(&self) -> Option<&Url> {
        self.discord.as_ref()
    }
    pub fn onlyfans(&self) -> Option<&Url> {
        self.onlyfans.as_ref()
    }
    pub fn others(&self) -> &HashMap<String, Url> {
        &self.others
    }

    // --- Fluent Setters (Immutable update pattern) ---

    pub fn with_website(mut self, url: Url) -> Self {
        self.website = Some(url);
        self
    }
    pub fn with_linkedin(mut self, url: Url) -> Self {
        self.linkedin = Some(url);
        self
    }
    pub fn with_github(mut self, url: Url) -> Self {
        self.github = Some(url);
        self
    }
    pub fn with_x(mut self, url: Url) -> Self {
        self.x = Some(url);
        self
    }
    pub fn with_instagram(mut self, url: Url) -> Self {
        self.instagram = Some(url);
        self
    }
    pub fn with_facebook(mut self, url: Url) -> Self {
        self.facebook = Some(url);
        self
    }
    pub fn with_tiktok(mut self, url: Url) -> Self {
        self.tiktok = Some(url);
        self
    }
    pub fn with_youtube(mut self, url: Url) -> Self {
        self.youtube = Some(url);
        self
    }
    pub fn with_twitch(mut self, url: Url) -> Self {
        self.twitch = Some(url);
        self
    }
    pub fn with_discord(mut self, url: Url) -> Self {
        self.discord = Some(url);
        self
    }
    pub fn with_onlyfans(mut self, url: Url) -> Self {
        self.onlyfans = Some(url);
        self
    }
    pub fn with_other(mut self, key: String, url: Url) -> Self {
        self.others.insert(key, url);
        self
    }

    pub fn validate(&self) -> Result<()> {
        if self.others.len() > 10 {
            return Err(Error::validation(
                "social_links",
                "Too many custom links (max 10)",
            ));
        }
        Ok(())
    }

    pub fn try_build(mut self) -> Result<Option<Self>> {
        // 1. Validation : Règle métier sur le nombre de liens personnalisés
        if self.others.len() > 10 {
            return Err(Error::validation(
                "social_links",
                "Too many custom links (max 10)",
            ));
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

impl From<serde_json::Value> for Socials {
    fn from(value: serde_json::Value) -> Self {
        serde_json::from_value(value).unwrap_or_default()
    }
}

impl From<Socials> for serde_json::Value {
    fn from(links: Socials) -> Self {
        serde_json::to_value(links).unwrap_or(serde_json::json!({}))
    }
}
