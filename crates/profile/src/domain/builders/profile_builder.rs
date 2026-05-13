// crates/profile/src/domain/builders/profile_builder.rs

use crate::entities::Profile;
use crate::value_objects::{Bio, DisplayName, Handle, Location, ProfileId, Socials};
use chrono::{DateTime, Utc};
use shared_kernel::core::{AggregateMetadata, Result};
use shared_kernel::types::{AccountId, Url};

pub struct ProfileBuilder {
    profile_id: ProfileId,
    account_id: AccountId,
    display_name: DisplayName,
    handle: Handle,
    bio: Option<Bio>,
    avatar_url: Option<Url>,
    banner_url: Option<Url>,
    location_label: Option<Location>,
    social_links: Option<Socials>,
    is_private: bool,
    created_at: Option<DateTime<Utc>>,
}

impl ProfileBuilder {
    pub(crate) fn new(account_id: AccountId, handle: Handle) -> Result<Self> {
        let display_name = DisplayName::try_new(handle.as_str())?;

        Ok(Self {
            profile_id: ProfileId::generate(),
            account_id,
            display_name,
            handle,
            bio: None,
            avatar_url: None,
            banner_url: None,
            location_label: None,
            social_links: None,
            is_private: false,
            created_at: None,
        })
    }

    // --- SETTERS ---

    pub fn with_profile_id(mut self, id: ProfileId) -> Self {
        self.profile_id = id;
        self
    }

    pub fn with_display_name(mut self, display_name: DisplayName) -> Self {
        self.display_name = display_name;
        self
    }

    pub fn with_bio(mut self, bio: Bio) -> Self {
        self.bio = Some(bio);
        self
    }

    pub fn with_avatar(mut self, url: Url) -> Self {
        self.avatar_url = Some(url);
        self
    }

    pub fn with_banner(mut self, url: Url) -> Self {
        self.banner_url = Some(url);
        self
    }

    pub fn with_location(mut self, label: Location) -> Self {
        self.location_label = Some(label);
        self
    }

    pub fn with_socials(mut self, links: Socials) -> Self {
        self.social_links = Some(links);
        self
    }

    pub fn with_privacy(mut self, private: bool) -> Self {
        self.is_private = private;
        self
    }

    pub fn with_created_at(mut self, date: DateTime<Utc>) -> Self {
        self.created_at = Some(date);
        self
    }

    /// Construit l'instance (Pure création mémoire)
    pub fn build(self) -> Result<Profile> {
        let now = Utc::now();

        let metadata = AggregateMetadata::default();

        Ok(Profile::restore(
            self.profile_id,
            self.account_id,
            self.display_name,
            self.handle,
            self.bio,
            self.avatar_url,
            self.banner_url,
            self.location_label,
            self.social_links,
            self.is_private,
            metadata.version(),
            self.created_at.unwrap_or(now),
            now,
        ))
    }
}
