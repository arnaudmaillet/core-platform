// crates/profile/src/domain/events.rs

use std::borrow::Cow;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;
use shared_kernel::domain::events::DomainEvent;
use shared_kernel::domain::value_objects::{LocationLabel, RegionCode, Url, AccountId};
use crate::domain::value_objects::SocialLinks;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ProfileEvent {
    /// Création initiale
    ProfileCreated {
        account_id: AccountId,
        region: RegionCode,
        display_name: String,
        username: String,
        occurred_at: DateTime<Utc>,
    },

    /// Changement de pseudonyme (Action critique : nécessite redirection d'URL)
    UsernameChanged {
        account_id: AccountId,
        old_username: String,
        new_username: String,
        occurred_at: DateTime<Utc>,
    },

    /// Mise à jour des médias (Utile pour le nettoyage de l'ancien cache CDN)
    MediaUpdated {
        account_id: AccountId,
        avatar_url: Option<Url>,
        banner_url: Option<Url>,
        occurred_at: DateTime<Utc>,
    },

    /// Changement de localisation ou bio
    MetadataUpdated {
        account_id: AccountId,
        display_name: String,
        bio: Option<String>,
        location: Option<LocationLabel>,
        occurred_at: DateTime<Utc>,
    },

    /// Mise à jour des réseaux sociaux
    SocialLinksUpdated {
        account_id: AccountId,
        links: SocialLinks,
        occurred_at: DateTime<Utc>,
    },

    /// Changement de confidentialité (Critique pour le moteur de recherche et le Feed)
    PrivacySettingsChanged {
        account_id: AccountId,
        region: RegionCode,
        is_private: bool,
        occurred_at: DateTime<Utc>,
    },

    PostCountIncremented {
        account_id: AccountId,
        post_id: Uuid,
        new_count: u64,
        occurred_at: DateTime<Utc>,
    },

    PostCountDecremented {
        account_id: AccountId,
        post_id: Uuid,
        new_count: u64,
        occurred_at: DateTime<Utc>,
    },

    /// Synchronisation des compteurs (Snapshot)
    StatsSnapshotUpdated {
        account_id: AccountId,
        follower_count: u64,
        following_count: u64,
        post_count: u64,
        occurred_at: DateTime<Utc>,
    },

    /// Suppression définitive
    ProfileDeleted {
        account_id: AccountId,
        region: RegionCode,
        occurred_at: DateTime<Utc>,
    },
}

impl DomainEvent for ProfileEvent {
    fn event_id(&self) -> Uuid {
        match self {
            Self::PostCountIncremented { post_id, .. } => *post_id,
            Self::PostCountDecremented { post_id, .. } => *post_id,
            _ => Uuid::now_v7(),
        }
    }
    fn event_type(&self) -> Cow<'_, str> {
        match self {
            Self::ProfileCreated { .. } => Cow::Borrowed("profile.created"),
            Self::UsernameChanged { .. } => Cow::Borrowed("profile.username.changed"),
            Self::MediaUpdated { .. } => Cow::Borrowed("profile.media.updated"),
            Self::MetadataUpdated { .. } => Cow::Borrowed("profile.metadata.updated"),
            Self::SocialLinksUpdated { .. } => Cow::Borrowed("profile.social_links.updated"),
            Self::PrivacySettingsChanged { .. } => Cow::Borrowed("profile.privacy.changed"),
            Self::PostCountIncremented { .. } => Cow::Borrowed("profile.post_count.incremented"),
            Self::PostCountDecremented { .. } => Cow::Borrowed("profile.post_count.decremented"),
            Self::StatsSnapshotUpdated { .. } => Cow::Borrowed("profile.stats.snapshot"),
            Self::ProfileDeleted { .. } => Cow::Borrowed("profile.deleted"),
        }
    }

    fn aggregate_type(&self) -> Cow<'_, str> {
        Cow::Borrowed("profile")
    }

    fn aggregate_id(&self) -> String {
        match self {
            Self::ProfileCreated { account_id: account_id, .. } |
            Self::UsernameChanged { account_id: account_id, .. } |
            Self::MediaUpdated { account_id: account_id, .. } |
            Self::MetadataUpdated { account_id: account_id, .. } |
            Self::SocialLinksUpdated { account_id: account_id, .. } |
            Self::PrivacySettingsChanged { account_id: account_id, .. } |
            Self::StatsSnapshotUpdated { account_id: account_id, .. } |
            Self::PostCountIncremented { account_id: account_id, .. } |
            Self::PostCountDecremented { account_id: account_id, .. } |
            Self::ProfileDeleted { account_id, .. } => account_id.to_string(),
        }
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        match self {
            Self::ProfileCreated { occurred_at, .. } |
            Self::UsernameChanged { occurred_at, .. } |
            Self::MediaUpdated { occurred_at, .. } |
            Self::MetadataUpdated { occurred_at, .. } |
            Self::SocialLinksUpdated { occurred_at, .. } |
            Self::PrivacySettingsChanged { occurred_at, .. } |
            Self::StatsSnapshotUpdated { occurred_at, .. } |
            Self::PostCountIncremented { occurred_at, .. } |
            Self::PostCountDecremented { occurred_at, .. } |
            Self::ProfileDeleted { occurred_at, .. } => *occurred_at,
        }
    }

    fn payload(&self) -> Value {
        json!(self)
    }
}