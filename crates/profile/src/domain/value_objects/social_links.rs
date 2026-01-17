// crates/profile/src/domain/value_objects.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use shared_kernel::domain::value_objects::Url;


/// Liens sociaux flexibles via JSONB en base de données
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SocialLinks {
    // --- PROFESSIONNEL & WEB ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linkedin: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github: Option<Url>,

    // --- RÉSEAUX SOCIAUX MAJEURS ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instagram: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facebook: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tiktok: Option<Url>,

    // --- CONTENU & STREAMING ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub youtube: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twitch: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discord: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onlyfans: Option<Url>,

    // --- FLEXIBILITÉ TOTALE ---
    /// Capture tout réseau social émergent ou spécifique (ex: Mastodon, Threads, BlueSky)
    /// sans nécessiter une migration de base de données ou de code.
    #[serde(flatten)]
    pub others: HashMap<String, Url>,
}