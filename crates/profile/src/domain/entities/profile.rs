// crates/profile/src/domain/entities/profile.rs

use crate::domain::builders::ProfileBuilder;
use crate::domain::events::ProfileEvent;
use crate::domain::value_objects::{Bio, DisplayName, Handle, ProfileId, SocialLinks};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::domain::entities::{Entity, Versioned};
use shared_kernel::domain::events::{
    AggregateMetadata, AggregateRoot, DomainEvent, EventEmitter, OperationTracker,
};
use shared_kernel::domain::value_objects::{AccountId, LocationLabel, Url};
use shared_kernel::errors::Result;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Profile {
    profile_id: ProfileId,
    account_id: AccountId,
    display_name: DisplayName,
    handle: Handle,
    bio: Option<Bio>,
    avatar_url: Option<Url>,
    banner_url: Option<Url>,
    location_label: Option<LocationLabel>,
    social_links: Option<SocialLinks>,
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
    fn push_event(&mut self, event: Box<dyn DomainEvent>) {
        self.metadata.push_event(event);
    }
    fn pull_events(&mut self) -> Vec<Box<dyn DomainEvent>> {
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
    pub fn builder(
        account_id: AccountId,
        display_name: DisplayName,
        handle: Handle,
    ) -> ProfileBuilder {
        ProfileBuilder::new(account_id, display_name, handle)
    }

    pub(crate) fn restore(
        profile_id: ProfileId,
        account_id: AccountId,
        display_name: DisplayName,
        handle: Handle,
        bio: Option<Bio>,
        avatar_url: Option<Url>,
        banner_url: Option<Url>,
        location_label: Option<LocationLabel>,
        social_links: Option<SocialLinks>,
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
            avatar_url,
            banner_url,
            location_label,
            social_links,
            is_private,
            created_at,
            metadata: AggregateMetadata::restore(version, updated_at),
        }
    }

    // --- GETTERS ---

    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }
    pub fn account_id(&self) -> &AccountId {
        &self.account_id
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
    pub fn avatar_url(&self) -> Option<&Url> {
        self.avatar_url.as_ref()
    }
    pub fn banner_url(&self) -> Option<&Url> {
        self.banner_url.as_ref()
    }
    pub fn location_label(&self) -> Option<&LocationLabel> {
        self.location_label.as_ref()
    }
    pub fn social_links(&self) -> Option<&SocialLinks> {
        self.social_links.as_ref()
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

    fn create_event_id() -> Uuid {
        Uuid::now_v7()
    }

    // --- MUTATEURS MÉTIER

    pub fn register(&mut self) -> Result<bool> {
        self.track_change(
            |_s| Ok(true),
            |s| {
                Box::new(ProfileEvent::ProfileCreated {
                    id: Uuid::now_v7(),
                    profile_id: s.profile_id.clone(),
                    account_id: s.account_id.clone(),
                    display_name: s.display_name.clone(),
                    handle: s.handle.clone(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn update_handle(&mut self, new_handle: Handle) -> Result<bool> {
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
                    profile_id: s.profile_id.clone(),
                    account_id: s.account_id.clone(),
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
                Box::new(ProfileEvent::DisplayNameChanged {
                    id: Uuid::now_v7(),
                    profile_id: s.profile_id.clone(),
                    account_id: s.account_id.clone(),
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
                    profile_id: s.profile_id.clone(),
                    account_id: s.account_id.clone(),
                    old_bio,
                    new_bio: s.bio.clone(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn update_avatar(&mut self, new_avatar_url: Url) -> Result<bool> {
        let old_avatar_url = self.avatar_url.clone();

        self.track_change(
            |s| {
                if s.avatar_url == Some(new_avatar_url.clone()) {
                    return Ok(false);
                }
                s.avatar_url = Some(new_avatar_url);
                Ok(true)
            },
            |s| {
                Box::new(ProfileEvent::AvatarUpdated {
                    id: Uuid::now_v7(),
                    profile_id: s.profile_id.clone(),
                    account_id: s.account_id.clone(),
                    old_avatar_url,
                    new_avatar_url: s.avatar_url.clone().unwrap(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn remove_avatar(&mut self) -> Result<bool> {
        if self.avatar_url.is_none() {
            return Ok(false);
        }

        let old_avatar_url = self.avatar_url.clone();

        self.track_change(
            |s| {
                s.avatar_url = None;
                Ok(true)
            },
            |s| {
                Box::new(ProfileEvent::AvatarRemoved {
                    id: Uuid::now_v7(),
                    profile_id: s.profile_id.clone(),
                    account_id: s.account_id.clone(),
                    old_avatar_url,
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn update_banner(&mut self, new_banner_url: Url) -> Result<bool> {
        if self.banner_url == Some(new_banner_url.clone()) {
            return Ok(false);
        }

        let old_banner_url = self.banner_url.clone();

        self.track_change(
            |s| {
                s.banner_url = Some(new_banner_url);
                Ok(true)
            },
            |s| {
                Box::new(ProfileEvent::BannerUpdated {
                    id: Uuid::now_v7(),
                    profile_id: s.profile_id.clone(),
                    account_id: s.account_id.clone(),
                    old_banner_url,
                    new_banner_url: s.banner_url.clone().unwrap(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn remove_banner(&mut self) -> Result<bool> {
        if self.banner_url.is_none() {
            return Ok(false);
        }

        let old_banner_url = self.banner_url.clone();

        self.track_change(
            |s| {
                s.banner_url = None;
                Ok(true)
            },
            |s| {
                Box::new(ProfileEvent::BannerRemoved {
                    id: Uuid::now_v7(),
                    profile_id: s.profile_id.clone(),
                    account_id: s.account_id.clone(),
                    old_banner_url,
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    /// Mise à jour des Metadata
    pub fn update_social_links(&mut self, new_links: Option<SocialLinks>) -> Result<bool> {
        if self.social_links == new_links {
            return Ok(false);
        }

        let old_links = self.social_links.clone();

        self.track_change(
            |s| {
                s.social_links = new_links;
                Ok(true)
            },
            |s| {
                Box::new(ProfileEvent::SocialLinksUpdated {
                    id: Uuid::now_v7(),
                    profile_id: s.profile_id.clone(),
                    account_id: s.account_id.clone(),
                    old_links,
                    new_links: s.social_links.clone(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn update_location_label(&mut self, new_label: Option<LocationLabel>) -> Result<bool> {
        if self.location_label == new_label {
            return Ok(false);
        }

        let old_location = self.location_label.clone();

        self.track_change(
            |s| {
                s.location_label = new_label;
                Ok(true)
            },
            |s| {
                Box::new(ProfileEvent::LocationLabelUpdated {
                    id: Uuid::now_v7(),
                    profile_id: s.profile_id.clone(),
                    account_id: s.account_id.clone(),
                    old_location,
                    new_location: s.location_label.clone(),
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
                Box::new(ProfileEvent::PrivacySettingsChanged {
                    id: Uuid::now_v7(),
                    profile_id: s.profile_id.clone(),
                    account_id: s.account_id.clone(),
                    is_private,
                    occurred_at: s.updated_at(),
                })
            },
        )
    }
}
