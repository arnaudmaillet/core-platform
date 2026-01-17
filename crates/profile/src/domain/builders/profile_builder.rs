// crates/profile/src/domain/builders/profile_builder.rs

use chrono::{DateTime, Utc};
use shared_kernel::domain::events::AggregateMetadata;
use shared_kernel::domain::value_objects::{Counter, LocationLabel, RegionCode, Url, AccountId, Username};
use crate::domain::entities::Profile;
use crate::domain::value_objects::{Bio, DisplayName, ProfileStats, SocialLinks};

pub struct ProfileBuilder {
    account_id: AccountId,
    region_code: RegionCode,
    display_name: DisplayName,
    username: Username,
    bio: Option<Bio>,
    avatar_url: Option<Url>,
    banner_url: Option<Url>,
    location_label: Option<LocationLabel>,
    social_links: SocialLinks,
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
        username: Username
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
            social_links: SocialLinks::default(),
            stats: ProfileStats::default(),
            post_count: Counter::default(),
            is_private: false,
            version: 1, // Par défaut pour un nouveau
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
        social_links: SocialLinks,
        post_count: Counter,
        is_private: bool,
        version: i32,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Profile {
        Profile {
            account_id: account_id,
            region_code,
            display_name,
            username,
            bio,
            avatar_url,
            banner_url,
            location_label,
            social_links,
            stats: ProfileStats::default(),
            post_count,
            is_private,
            created_at,
            updated_at,
            metadata: AggregateMetadata::restore(version),
        }
    }

    // --- SETTERS (Uniquement utiles pour le chemin Création) ---

    pub fn bio(mut self, bio: Bio) -> Self { self.bio = Some(bio); self }
    pub fn maybe_bio(mut self, bio: Option<Bio>) -> Self { self.bio = bio; self }
    pub fn avatar_url(mut self, url: Url) -> Self { self.avatar_url = Some(url); self }
    pub fn maybe_avatar_url(mut self, url: Option<Url>) -> Self { self.avatar_url = url; self }
    pub fn banner_url(mut self, url: Url) -> Self { self.banner_url = Some(url); self }
    pub fn maybe_banner_url(mut self, url: Option<Url>) -> Self { self.banner_url = url; self }
    pub fn location(mut self, label: LocationLabel) -> Self { self.location_label = Some(label); self }
    pub fn maybe_location(mut self, label: Option<LocationLabel>) -> Self { self.location_label = label; self }
    pub fn social_links(mut self, links: SocialLinks) -> Self { self.social_links = links; self }
    pub fn stats(mut self, stats: ProfileStats) -> Self { self.stats = stats; self }
    pub fn post_count(mut self, count: Counter) -> Self { self.post_count = count; self }
    pub fn is_private(mut self, private: bool) -> Self { self.is_private = private; self }

    /// Finalisation pour la CREATION
    pub fn build(self) -> Profile {
        let now = Utc::now();
        Profile {
            account_id: self.account_id,
            region_code: self.region_code,
            display_name: self.display_name,
            username: self.username,
            bio: self.bio,
            avatar_url: self.avatar_url,
            banner_url: self.banner_url,
            location_label: self.location_label,
            social_links: self.social_links,
            stats: self.stats,
            post_count: self.post_count,
            is_private: self.is_private,
            created_at: self.created_at.unwrap_or(now),
            updated_at: now,
            metadata: AggregateMetadata::new(self.version),
        }
    }
}