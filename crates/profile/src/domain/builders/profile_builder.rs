// crates/profile/src/domain/builders/profile_builder.rs

use crate::domain::entities::Profile;
use crate::domain::events::ProfileEvent;
use crate::domain::value_objects::{Bio, DisplayName, ProfileStats, SocialLinks};
use chrono::{DateTime, Utc};
use shared_kernel::domain::events::{AggregateMetadata, AggregateRoot};
use shared_kernel::domain::value_objects::{
    AccountId, Counter, LocationLabel, RegionCode, Url, Username,
};
use uuid::Uuid;

pub struct ProfileBuilder {
    account_id: AccountId,
    region_code: RegionCode,
    display_name: DisplayName,
    username: Username,
    bio: Option<Bio>,
    avatar_url: Option<Url>,
    banner_url: Option<Url>,
    location_label: Option<LocationLabel>,
    social_links: Option<SocialLinks>,
    stats: ProfileStats,
    post_count: Counter,
    is_private: bool,
    version: i32,
    created_at: Option<DateTime<Utc>>,
}

impl ProfileBuilder {
    /// Chemin 1 : CREATION (Via Use Case / API)
    pub fn new(
        account_id: AccountId,
        region_code: RegionCode,
        display_name: DisplayName,
        username: Username,
    ) -> Self {
        Self {
            account_id,
            region_code,
            display_name,
            username,
            bio: None,
            avatar_url: None,
            banner_url: None,
            location_label: None,
            social_links: None,
            stats: ProfileStats::default(),
            post_count: Counter::default(),
            is_private: false,
            version: 1,
            created_at: None,
        }
    }

    /// Chemin 2 : RESTAURATION (Via Infrastructure / Repository)
    /// Direct et sans détour pour la performance SQL
    pub fn restore(
        account_id: AccountId,
        region_code: RegionCode,
        display_name: DisplayName,
        username: Username,
        bio: Option<Bio>,
        avatar_url: Option<Url>,
        banner_url: Option<Url>,
        location_label: Option<LocationLabel>,
        social_links: Option<SocialLinks>,
        post_count: Counter,
        is_private: bool,
        version: i32,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Profile {
        Profile::restore(
            account_id,
            region_code,
            display_name,
            username,
            bio,
            avatar_url,
            banner_url,
            location_label,
            social_links,
            post_count,
            is_private,
            version,
            created_at,
            updated_at,
        )
    }
    // --- SETTERS (Uniquement utiles pour le chemin Création) ---

    pub fn with_bio(mut self, bio: Bio) -> Self {
        self.bio = Some(bio);
        self
    }
    pub fn with_optional_bio(mut self, bio: Option<Bio>) -> Self {
        self.bio = bio;
        self
    }
    pub fn with_avatar_url(mut self, url: Url) -> Self {
        self.avatar_url = Some(url);
        self
    }
    pub fn with_optional_avatar_url(mut self, url: Option<Url>) -> Self {
        self.avatar_url = url;
        self
    }
    pub fn with_banner_url(mut self, url: Url) -> Self {
        self.banner_url = Some(url);
        self
    }
    pub fn with_optional_banner_url(mut self, url: Option<Url>) -> Self {
        self.banner_url = url;
        self
    }
    pub fn with_location(mut self, label: LocationLabel) -> Self {
        self.location_label = Some(label);
        self
    }
    pub fn with_optional_location(mut self, label: Option<LocationLabel>) -> Self {
        self.location_label = label;
        self
    }
    pub fn with_optional_social_links(mut self, links: Option<SocialLinks>) -> Self {
        self.social_links = links;
        self
    }
    pub fn with_stats(mut self, stats: ProfileStats) -> Self {
        self.stats = stats;
        self
    }
    pub fn with_post_count(mut self, count: Counter) -> Self {
        self.post_count = count;
        self
    }
    pub fn with_privacy(mut self, private: bool) -> Self {
        self.is_private = private;
        self
    }

    /// Finalisation pour la CREATION
    pub fn build(self) -> Profile {
        let now = Utc::now();
        Profile::new_from_builder(
            self.account_id,
            self.region_code,
            self.display_name,
            self.username,
            self.bio,
            self.avatar_url,
            self.banner_url,
            self.location_label,
            self.social_links,
            self.stats,
            self.post_count,
            self.is_private,
            self.created_at.unwrap_or(now),
            now,
            self.version,
        )
    }

    pub fn build_new(self) -> Profile {
        let mut profile = self.build(); // Utilise ton build() actuel

        // On enregistre l'événement de création dans l'agrégat
        profile.add_event(Box::new(ProfileEvent::ProfileCreated {
            id: Uuid::now_v7(),
            account_id: profile.account_id().clone(),
            region: profile.region_code().clone(),
            display_name: profile.display_name().clone(),
            username: profile.username().clone(),
            occurred_at: profile.created_at(),
        }));

        profile
    }
}
