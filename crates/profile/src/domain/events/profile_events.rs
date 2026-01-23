// crates/profile/src/domain/events.rs

use std::borrow::Cow;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;
use shared_kernel::domain::events::DomainEvent;
use shared_kernel::domain::value_objects::{LocationLabel, RegionCode, Url, AccountId, Username};
use crate::domain::value_objects::{Bio, DisplayName, SocialLinks};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ProfileEvent {
    /// Création initiale
    ProfileCreated {
        id: Uuid,
        account_id: AccountId,
        region: RegionCode,
        display_name: DisplayName,
        username: Username,
        occurred_at: DateTime<Utc>,
    },

    /// Changement de pseudonyme (Action critique : nécessite redirection d'URL)
    UsernameChanged {
        id: Uuid,
        account_id: AccountId,
        old_username: Username,
        new_username: Username,
        occurred_at: DateTime<Utc>,
    },

    DisplayNameChanged {
        id: Uuid,
        account_id: AccountId,
        old_display_name: DisplayName,
        new_display_name: DisplayName,
        occurred_at: DateTime<Utc>,
    },

    /// Mise à jour des médias (Utile pour le nettoyage de l'ancien cache CDN)
    AvatarUpdated {
        id: Uuid,
        account_id: AccountId,
        old_avatar_url: Option<Url>,
        new_avatar_url: Url,
        occurred_at: DateTime<Utc>,
    },

    AvatarRemoved {
        id: Uuid,
        account_id: AccountId,
        old_avatar_url: Option<Url>,
        occurred_at: DateTime<Utc>,
    },

    BannerUpdated {
        id: Uuid,
        account_id: AccountId,
        old_banner_url: Option<Url>,
        new_banner_url: Url,
        occurred_at: DateTime<Utc>,
    },

    BannerRemoved {
        id: Uuid,
        account_id: AccountId,
        old_banner_url: Option<Url>,
        occurred_at: DateTime<Utc>,
    },

    BioUpdated {
        id: Uuid,
        account_id: AccountId,
        old_bio: Option<Bio>,
        new_bio: Option<Bio>,
        occurred_at: DateTime<Utc>,
    },


    LocationLabelUpdated {
        id: Uuid,
        account_id: AccountId,
        old_location: Option<LocationLabel>,
        new_location: Option<LocationLabel>,
        occurred_at: DateTime<Utc>,
    },

    /// Mise à jour des réseaux sociaux
    SocialLinksUpdated {
        id: Uuid,
        account_id: AccountId,
        old_links: Option<SocialLinks>,
        new_links: Option<SocialLinks>,
        occurred_at: DateTime<Utc>,
    },

    /// Changement de confidentialité (Critique pour le moteur de recherche et le Feed)
    PrivacySettingsChanged {
        id: Uuid,
        account_id: AccountId,
        region: RegionCode,
        is_private: bool,
        occurred_at: DateTime<Utc>,
    },

    PostCountIncremented {
        id: Uuid,
        account_id: AccountId,
        post_id: Uuid,
        new_count: u64,
        occurred_at: DateTime<Utc>,
    },

    PostCountDecremented {
        id: Uuid,
        account_id: AccountId,
        post_id: Uuid,
        new_count: u64,
        occurred_at: DateTime<Utc>,
    },

    /// Synchronisation des compteurs (Snapshot)
    StatsSnapshotUpdated {
        id: Uuid,
        account_id: AccountId,
        follower_count: u64,
        following_count: u64,
        post_count: u64,
        occurred_at: DateTime<Utc>,
    },

    /// Suppression définitive
    ProfileDeleted {
        id: Uuid,
        account_id: AccountId,
        region: RegionCode,
        occurred_at: DateTime<Utc>,
    },
}

impl DomainEvent for ProfileEvent {
    fn event_id(&self) -> Uuid {
        match self {
            Self::PostCountIncremented { id, .. } |
            Self::PostCountDecremented { id, .. } |
            Self::ProfileCreated { id, .. } |
            Self::UsernameChanged { id, .. } |
            Self::DisplayNameChanged { id, .. } |
            Self::AvatarUpdated { id, .. } |
            Self::AvatarRemoved { id, .. } |
            Self::BannerUpdated { id, .. } |
            Self::BannerRemoved { id, .. } |
            Self::BioUpdated { id, .. } |
            Self::LocationLabelUpdated { id, .. } |
            Self::SocialLinksUpdated { id, .. } |
            Self::PrivacySettingsChanged { id, .. } |
            Self::StatsSnapshotUpdated { id, .. } |
            Self::ProfileDeleted { id, .. } => *id,
        }
    }
    fn event_type(&self) -> Cow<'_, str> {
        match self {
            Self::ProfileCreated { .. } => Cow::Borrowed("profile.created"),
            Self::UsernameChanged { .. } => Cow::Borrowed("profile.username.changed"),
            Self::DisplayNameChanged { .. } => Cow::Borrowed("profile.displayname.changed"),
            Self::AvatarUpdated { .. } => Cow::Borrowed("profile.avatar.changed"),
            Self::AvatarRemoved { .. } => Cow::Borrowed("profile.avatar.removed"),
            Self::BannerUpdated { .. } => Cow::Borrowed("profile.banner.changed"),
            Self::BannerRemoved { .. } => Cow::Borrowed("profile.banner.removed"),
            Self::BioUpdated { .. } => Cow::Borrowed("profile.bio.updated"),
            Self::LocationLabelUpdated { .. } => Cow::Borrowed("profile.location_label.updated"),
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
            Self::ProfileCreated { account_id, .. } |
            Self::UsernameChanged { account_id, .. } |
            Self::DisplayNameChanged { account_id, .. } |
            Self::AvatarUpdated { account_id, .. } |
            Self::AvatarRemoved { account_id, .. } |
            Self::BannerUpdated { account_id, .. } |
            Self::BannerRemoved { account_id, .. } |
            Self::BioUpdated { account_id, .. } |
            Self::LocationLabelUpdated { account_id, .. } |
            Self::SocialLinksUpdated { account_id, .. } |
            Self::PrivacySettingsChanged { account_id, .. } |
            Self::StatsSnapshotUpdated { account_id, .. } |
            Self::PostCountIncremented { account_id, .. } |
            Self::PostCountDecremented { account_id, .. } |
            Self::ProfileDeleted { account_id, .. } => account_id.to_string(),
        }
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        match self {
            Self::ProfileCreated { occurred_at, .. } |
            Self::UsernameChanged { occurred_at, .. } |
            Self::DisplayNameChanged { occurred_at, .. } |
            Self::AvatarUpdated { occurred_at, .. } |
            Self::AvatarRemoved { occurred_at, .. } |
            Self::BannerUpdated { occurred_at, .. } |
            Self::BannerRemoved { occurred_at, .. } |
            Self::BioUpdated { occurred_at, .. } |
            Self::LocationLabelUpdated { occurred_at, .. } |
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