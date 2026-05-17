// crates/profile/src/domain/entities/profile.rs

use crate::entities::ProfileBuilder;
use crate::events::ProfileEvent;
use crate::types::{Bio, DisplayName, Handle, Location, Socials};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::core::{AggregateMetadata, AggregateRoot, Result};
use shared_kernel::core::{Entity, Versioned};
use shared_kernel::messaging::{Event, EventEmitter, OperationTracker};
use shared_kernel::types::{AccountId, ProfileId, Url};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Profile {
    profile_id: ProfileId,
    account_id: AccountId,
    display_name: DisplayName,
    handle: Handle,
    bio: Option<Bio>,
    avatar: Option<Url>,
    banner: Option<Url>,
    location: Option<Location>,
    socials: Option<Socials>,
    is_private: bool,
    created_at: DateTime<Utc>,
    metadata: AggregateMetadata,
}

impl Versioned for Profile {
    fn version(&self) -> u64 {
        self.metadata.version()
    }
    fn updated_at(&self) -> DateTime<Utc> {
        self.metadata.updated_at()
    }
    fn record_change(&mut self) {
        self.metadata.record_change();
    }
}

impl EventEmitter for Profile {
    fn push_event(&mut self, event: Box<dyn Event>) {
        self.metadata.push_event(event);
    }
    fn pull_events(&mut self) -> Vec<Box<dyn Event>> {
        self.metadata.pull_events()
    }
}

impl AggregateRoot for Profile {
    fn id(&self) -> String {
        self.profile_id.to_string()
    }
    fn metadata(&self) -> &AggregateMetadata {
        &self.metadata
    }
    fn metadata_mut(&mut self) -> &mut AggregateMetadata {
        &mut self.metadata
    }
}

impl Entity for Profile {
    type Id = ProfileId;

    // Métadonnées
    fn entity_name() -> &'static str {
        "Profile"
    }

    fn map_constraint_to_field(constraint: &str) -> &'static str {
        match constraint {
            "account_governance_pkey" => "account_id",
            _ => "internal_governance",
        }
    }

    fn id(&self) -> &Self::Id {
        &self.profile_id
    }
    fn updated_at(&self) -> DateTime<Utc> {
        self.metadata.updated_at()
    }
}

impl Profile {
    pub fn builder(account_id: AccountId, handle: Handle) -> Result<ProfileBuilder> {
        ProfileBuilder::new(account_id, handle)
    }

    pub(crate) fn restore(
        profile_id: ProfileId,
        account_id: AccountId,
        display_name: DisplayName,
        handle: Handle,
        bio: Option<Bio>,
        avatar: Option<Url>,
        banner: Option<Url>,
        location: Option<Location>,
        socials: Option<Socials>,
        is_private: bool,
        version: u64,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Profile {
        Profile {
            profile_id,
            account_id,
            display_name,
            handle,
            bio,
            avatar,
            banner,
            location,
            socials,
            is_private,
            created_at,
            metadata: AggregateMetadata::restore(version, updated_at),
        }
    }

    // --- GETTERS ---

    pub fn profile_id(&self) -> ProfileId {
        self.profile_id
    }
    pub fn account_id(&self) -> AccountId {
        self.account_id
    }
    pub fn display_name(&self) -> &DisplayName {
        &self.display_name
    }
    pub fn handle(&self) -> &Handle {
        &self.handle
    }
    pub fn bio(&self) -> Option<&Bio> {
        self.bio.as_ref()
    }
    pub fn avatar(&self) -> Option<&Url> {
        self.avatar.as_ref()
    }
    pub fn banner(&self) -> Option<&Url> {
        self.banner.as_ref()
    }
    pub fn location(&self) -> Option<&Location> {
        self.location.as_ref()
    }
    pub fn socials(&self) -> Option<&Socials> {
        self.socials.as_ref()
    }
    pub fn is_private(&self) -> bool {
        self.is_private
    }
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    pub fn updated_at(&self) -> DateTime<Utc> {
        Versioned::updated_at(self)
    }

    // --- MUTATEURS MÉTIER

    pub fn create_profile(&mut self) -> Result<bool> {
        self.track_change(
            |_s| Ok(true),
            |s| {
                Box::new(ProfileEvent::ProfileCreated {
                    id: Uuid::now_v7(),
                    profile_id: s.profile_id,
                    account_id: s.account_id,
                    display_name: s.display_name.clone(),
                    handle: s.handle.clone(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn change_handle(&mut self, new_handle: Handle) -> Result<bool> {
        if self.handle == new_handle {
            return Ok(false);
        }

        let old_handle = self.handle.clone();

        self.track_change(
            |s| {
                s.handle = new_handle;
                Ok(true)
            },
            |s| {
                Box::new(ProfileEvent::HandleChanged {
                    id: Uuid::now_v7(),
                    profile_id: s.profile_id,
                    account_id: s.account_id,
                    old_handle,
                    new_handle: s.handle.clone(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn update_display_name(&mut self, new_name: DisplayName) -> Result<bool> {
        if self.display_name == new_name {
            return Ok(false);
        }

        let old_display_name = self.display_name.clone();

        self.track_change(
            |s| {
                s.display_name = new_name;
                Ok(true)
            },
            |s| {
                Box::new(ProfileEvent::DisplayNameUpdated {
                    id: Uuid::now_v7(),
                    profile_id: s.profile_id,
                    account_id: s.account_id,
                    old_display_name,
                    new_display_name: s.display_name.clone(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn update_bio(&mut self, new_bio: Option<Bio>) -> Result<bool> {
        if self.bio == new_bio {
            return Ok(false);
        }

        let old_bio = self.bio.clone();

        self.track_change(
            |s| {
                s.bio = new_bio;
                Ok(true)
            },
            |s| {
                Box::new(ProfileEvent::BioUpdated {
                    id: Uuid::now_v7(),
                    profile_id: s.profile_id,
                    account_id: s.account_id,
                    old_bio,
                    new_bio: s.bio.clone(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn update_avatar(&mut self, new_avatar_url: Url) -> Result<bool> {
        let old_avatar_url = self.avatar.clone();

        self.track_change(
            |s| {
                if s.avatar == Some(new_avatar_url.clone()) {
                    return Ok(false);
                }
                s.avatar = Some(new_avatar_url);
                Ok(true)
            },
            |s| {
                Box::new(ProfileEvent::AvatarUpdated {
                    id: Uuid::now_v7(),
                    profile_id: s.profile_id,
                    account_id: s.account_id,
                    old_avatar_url,
                    new_avatar_url: s.avatar.clone().unwrap(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn remove_avatar(&mut self) -> Result<bool> {
        if self.avatar.is_none() {
            return Ok(false);
        }

        let old_avatar_url = self.avatar.clone();

        self.track_change(
            |s| {
                s.avatar = None;
                Ok(true)
            },
            |s| {
                Box::new(ProfileEvent::AvatarRemoved {
                    id: Uuid::now_v7(),
                    profile_id: s.profile_id,
                    account_id: s.account_id,
                    old_avatar_url,
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn update_banner(&mut self, new_banner_url: Url) -> Result<bool> {
        if self.banner == Some(new_banner_url.clone()) {
            return Ok(false);
        }

        let old_banner_url = self.banner.clone();

        self.track_change(
            |s| {
                s.banner = Some(new_banner_url);
                Ok(true)
            },
            |s| {
                Box::new(ProfileEvent::BannerUpdated {
                    id: Uuid::now_v7(),
                    profile_id: s.profile_id,
                    account_id: s.account_id,
                    old_banner_url,
                    new_banner_url: s.banner.clone().unwrap(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn remove_banner(&mut self) -> Result<bool> {
        if self.banner.is_none() {
            return Ok(false);
        }

        let old_banner_url = self.banner.clone();

        self.track_change(
            |s| {
                s.banner = None;
                Ok(true)
            },
            |s| {
                Box::new(ProfileEvent::BannerRemoved {
                    id: Uuid::now_v7(),
                    profile_id: s.profile_id,
                    account_id: s.account_id,
                    old_banner_url,
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    /// Mise à jour des Metadata
    pub fn update_socials(&mut self, new_socials: Option<Socials>) -> Result<bool> {
        if self.socials == new_socials {
            return Ok(false);
        }

        let old_socials = self.socials.clone();

        self.track_change(
            |s| {
                s.socials = new_socials;
                Ok(true)
            },
            |s| {
                Box::new(ProfileEvent::SocialsUpdated {
                    id: Uuid::now_v7(),
                    profile_id: s.profile_id,
                    account_id: s.account_id,
                    old_socials,
                    new_socials: s.socials.clone(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn update_location(&mut self, new_label: Option<Location>) -> Result<bool> {
        if self.location == new_label {
            return Ok(false);
        }

        let old_location = self.location.clone();

        self.track_change(
            |s| {
                s.location = new_label;
                Ok(true)
            },
            |s| {
                Box::new(ProfileEvent::LocationUpdated {
                    id: Uuid::now_v7(),
                    profile_id: s.profile_id,
                    account_id: s.account_id,
                    old_location,
                    new_location: s.location.clone(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn update_privacy(&mut self, is_private: bool) -> Result<bool> {
        if self.is_private == is_private {
            return Ok(false);
        }

        self.track_change(
            |s| {
                s.is_private = is_private;
                Ok(true)
            },
            |s| {
                Box::new(ProfileEvent::PrivacyChanged {
                    id: Uuid::now_v7(),
                    profile_id: s.profile_id,
                    account_id: s.account_id,
                    is_private,
                    occurred_at: s.updated_at(),
                })
            },
        )
    }
}
