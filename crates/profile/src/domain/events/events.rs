// crates/profile/src/domain/events.rs

use crate::types::{Bio, DisplayName, Handle, Location, Socials};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use shared_kernel::{
    messaging::Event,
    types::{AccountId, ProfileId, Url},
};
use std::borrow::Cow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ProfileEvent {
    /// Création initiale
    ProfileCreated {
        id: Uuid,
        profile_id: ProfileId,
        account_id: AccountId,
        display_name: DisplayName,
        handle: Handle,
        occurred_at: DateTime<Utc>,
    },

    HandleChanged {
        id: Uuid,
        profile_id: ProfileId,
        account_id: AccountId,
        old_handle: Handle,
        new_handle: Handle,
        occurred_at: DateTime<Utc>,
    },

    DisplayNameUpdated {
        id: Uuid,
        profile_id: ProfileId,
        account_id: AccountId,
        old_display_name: DisplayName,
        new_display_name: DisplayName,
        occurred_at: DateTime<Utc>,
    },

    AvatarUpdated {
        id: Uuid,
        profile_id: ProfileId,
        account_id: AccountId,
        old_avatar_url: Option<Url>,
        new_avatar_url: Url,
        occurred_at: DateTime<Utc>,
    },

    AvatarRemoved {
        id: Uuid,
        profile_id: ProfileId,
        account_id: AccountId,
        old_avatar_url: Option<Url>,
        occurred_at: DateTime<Utc>,
    },

    BannerUpdated {
        id: Uuid,
        profile_id: ProfileId,
        account_id: AccountId,
        old_banner_url: Option<Url>,
        new_banner_url: Url,
        occurred_at: DateTime<Utc>,
    },

    BannerRemoved {
        id: Uuid,
        profile_id: ProfileId,
        account_id: AccountId,
        old_banner_url: Option<Url>,
        occurred_at: DateTime<Utc>,
    },

    BioUpdated {
        id: Uuid,
        profile_id: ProfileId,
        account_id: AccountId,
        old_bio: Option<Bio>,
        new_bio: Option<Bio>,
        occurred_at: DateTime<Utc>,
    },

    LocationUpdated {
        id: Uuid,
        profile_id: ProfileId,
        account_id: AccountId,
        old_location: Option<Location>,
        new_location: Option<Location>,
        occurred_at: DateTime<Utc>,
    },

    SocialsUpdated {
        id: Uuid,
        profile_id: ProfileId,
        account_id: AccountId,
        old_socials: Option<Socials>,
        new_socials: Option<Socials>,
        occurred_at: DateTime<Utc>,
    },

    PrivacyChanged {
        id: Uuid,
        profile_id: ProfileId,
        account_id: AccountId,
        is_private: bool,
        occurred_at: DateTime<Utc>,
    },

    ProfileDeleted {
        id: Uuid,
        profile_id: ProfileId,
        account_id: AccountId,
        occurred_at: DateTime<Utc>,
    },
}

impl ProfileEvent {
    pub const PROFILE_CREATED: &'static str = "profile.created";
    pub const PROFILE_DELETED: &'static str = "profile.deleted";
    pub const HANDLE_CHANGED: &'static str = "profile.handle.changed";
    pub const DISPLAY_NAME_UPDATED: &'static str = "profile.display_name.updated";
    pub const AVATAR_UPDATED: &'static str = "profile.avatar.updated";
    pub const AVATAR_REMOVED: &'static str = "profile.avatar.removed";
    pub const BANNER_UPDATED: &'static str = "profile.banner.updated";
    pub const BANNER_REMOVED: &'static str = "profile.banner.removed";
    pub const BIO_UPDATED: &'static str = "profile.bio.updated";
    pub const LOCATION_UPDATED: &'static str = "profile.location.updated";
    pub const SOCIALS_UPDATED: &'static str = "profile.socials.updated";
    pub const PRIVACY_UPDATED: &'static str = "profile.privacy.updated";
}

impl Event for ProfileEvent {
    fn event_id(&self) -> Uuid {
        match self {
            Self::ProfileCreated { id, .. }
            | Self::HandleChanged { id, .. }
            | Self::DisplayNameUpdated { id, .. }
            | Self::AvatarUpdated { id, .. }
            | Self::AvatarRemoved { id, .. }
            | Self::BannerUpdated { id, .. }
            | Self::BannerRemoved { id, .. }
            | Self::BioUpdated { id, .. }
            | Self::LocationUpdated { id, .. }
            | Self::SocialsUpdated { id, .. }
            | Self::PrivacyChanged { id, .. }
            | Self::ProfileDeleted { id, .. } => *id,
        }
    }
    fn event_name(&self) -> Cow<'_, str> {
        let s = match self {
            Self::ProfileCreated { .. } => Self::PROFILE_CREATED,
            Self::HandleChanged { .. } => Self::HANDLE_CHANGED,
            Self::DisplayNameUpdated { .. } => Self::DISPLAY_NAME_UPDATED,
            Self::AvatarUpdated { .. } => Self::AVATAR_UPDATED,
            Self::AvatarRemoved { .. } => Self::AVATAR_REMOVED,
            Self::BannerUpdated { .. } => Self::BANNER_UPDATED,
            Self::BannerRemoved { .. } => Self::BANNER_REMOVED,
            Self::BioUpdated { .. } => Self::BIO_UPDATED,
            Self::LocationUpdated { .. } => Self::LOCATION_UPDATED,
            Self::SocialsUpdated { .. } => Self::SOCIALS_UPDATED,
            Self::PrivacyChanged { .. } => Self::PRIVACY_UPDATED,
            Self::ProfileDeleted { .. } => Self::PROFILE_DELETED,
        };
        Cow::Borrowed(s)
    }

    fn aggregate_type(&self) -> Cow<'_, str> {
        Cow::Borrowed("profile")
    }

    fn aggregate_id(&self) -> String {
        match self {
            Self::ProfileCreated { profile_id, .. }
            | Self::HandleChanged { profile_id, .. }
            | Self::DisplayNameUpdated { profile_id, .. }
            | Self::AvatarUpdated { profile_id, .. }
            | Self::AvatarRemoved { profile_id, .. }
            | Self::BannerUpdated { profile_id, .. }
            | Self::BannerRemoved { profile_id, .. }
            | Self::BioUpdated { profile_id, .. }
            | Self::LocationUpdated { profile_id, .. }
            | Self::SocialsUpdated { profile_id, .. }
            | Self::PrivacyChanged { profile_id, .. }
            | Self::ProfileDeleted { profile_id, .. } => profile_id.to_string(),
        }
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        match self {
            Self::ProfileCreated { occurred_at, .. }
            | Self::HandleChanged { occurred_at, .. }
            | Self::DisplayNameUpdated { occurred_at, .. }
            | Self::AvatarUpdated { occurred_at, .. }
            | Self::AvatarRemoved { occurred_at, .. }
            | Self::BannerUpdated { occurred_at, .. }
            | Self::BannerRemoved { occurred_at, .. }
            | Self::BioUpdated { occurred_at, .. }
            | Self::LocationUpdated { occurred_at, .. }
            | Self::SocialsUpdated { occurred_at, .. }
            | Self::PrivacyChanged { occurred_at, .. }
            | Self::ProfileDeleted { occurred_at, .. } => *occurred_at,
        }
    }

    fn payload(&self) -> Value {
        json!(self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
