// crates/profile/src/domain/entities/profile.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::domain::Identifier;
use shared_kernel::domain::entities::EntityMetadata;
use shared_kernel::domain::events::{AggregateMetadata, AggregateRoot};
use shared_kernel::domain::value_objects::{AccountId, Counter, LocationLabel, PostId, RegionCode, Url };
use uuid::Uuid;
use shared_kernel::errors::{DomainError, Result};
use crate::domain::builders::ProfileBuilder;
use crate::domain::events::ProfileEvent;
use crate::domain::value_objects::{Bio, DisplayName, Handle, ProfileId, ProfileStats, SocialLinks};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Profile {
    id: ProfileId,
    owner_id: AccountId,
    region_code: RegionCode,
    display_name: DisplayName,
    handle: Handle,
    bio: Option<Bio>,
    avatar_url: Option<Url>,
    banner_url: Option<Url>,
    location_label: Option<LocationLabel>,
    social_links: Option<SocialLinks>,
    stats: ProfileStats,
    post_count: Counter,
    is_private: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    metadata: AggregateMetadata,
}

impl Profile {
    pub fn builder(
        owner_id: AccountId,
        region_code: RegionCode,
        display_name: DisplayName,
        handle: Handle,
    ) -> ProfileBuilder {
        ProfileBuilder::new(owner_id, region_code, display_name, handle)
    }

    pub(crate) fn restore(
        id: ProfileId,
        owner_id: AccountId,
        region_code: RegionCode,
        display_name: DisplayName,
        handle: Handle,
        bio: Option<Bio>,
        avatar_url: Option<Url>,
        banner_url: Option<Url>,
        location_label: Option<LocationLabel>,
        social_links: Option<SocialLinks>,
        post_count: Counter,
        is_private: bool,
        version: u64,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Profile {
        Profile {
            id,
            owner_id,
            region_code,
            display_name,
            handle,
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

    // --- GETTERS ---

    pub fn id(&self) -> &ProfileId { &self.id }
    pub fn owner_id(&self) -> &AccountId { &self.owner_id }
    pub fn region_code(&self) -> &RegionCode { &self.region_code }
    pub fn display_name(&self) -> &DisplayName { &self.display_name }
    pub fn handle(&self) -> &Handle { &self.handle }
    pub fn bio(&self) -> Option<&Bio> { self.bio.as_ref() }
    pub fn avatar_url(&self) -> Option<&Url> { self.avatar_url.as_ref() }
    pub fn banner_url(&self) -> Option<&Url> { self.banner_url.as_ref() }
    pub fn location_label(&self) -> Option<&LocationLabel> { self.location_label.as_ref() }
    pub fn social_links(&self) -> Option<&SocialLinks> { self.social_links.as_ref() }
    pub fn stats(&self) -> &ProfileStats { &self.stats }
    pub fn post_count(&self) -> u64 { self.post_count.value() }
    pub fn is_private(&self) -> bool { self.is_private }
    pub fn created_at(&self) -> DateTime<Utc> { self.created_at }
    pub fn updated_at(&self) -> DateTime<Utc> { self.updated_at }

    fn create_event_id() -> Uuid {
        Uuid::now_v7()
    }

    // --- MUTATEURS MÉTIER

    pub fn create(mut profile: Self) -> Self {
        let occurred_at = profile.created_at();
        profile.add_event(Box::new(ProfileEvent::ProfileCreated {
            id: Uuid::now_v7(),
            profile_id: profile.id().clone(),
            owner_id: profile.owner_id().clone(),
            region: profile.region_code().clone(),
            display_name: profile.display_name().clone(),
            handle: profile.handle().clone(),
            occurred_at,
        }));

        profile
    }

    /// Mise à jour identity - Action Critique
    pub fn update_handle(&mut self, region: &RegionCode, new_handle: Handle) -> Result<bool> {
        self.ensure_region_match(region)?;
        if self.handle == new_handle {
            return Ok(false);
        }

        // let old_handle = self.handle.clone();
        // self.handle = new_handle;
        let old_handle = std::mem::replace(&mut self.handle, new_handle);

        self.apply_change();
        self.add_event(Box::new(ProfileEvent::HandleChanged {
            id: Self::create_event_id(),
            profile_id: self.id.clone(),
            owner_id: self.owner_id.clone(),
            region: self.region_code.clone(),
            old_handle,
            new_handle: self.handle.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn update_display_name(&mut self, region: &RegionCode, new_display_name: DisplayName) -> Result<bool> {
        self.ensure_region_match(region)?;

        if self.display_name == new_display_name {
            return Ok(false);
        }

        // let old_display_name = self.display_name.clone();
        // self.display_name = new_display_name.clone();
        let old_display_name = std::mem::replace(&mut self.display_name, new_display_name);

        self.apply_change();
        self.add_event(Box::new(ProfileEvent::DisplayNameChanged {
            id: Self::create_event_id(),
            profile_id: self.id.clone(),
            owner_id: self.owner_id.clone(),
            region: self.region_code.clone(),
            old_display_name,
            new_display_name: self.display_name.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn update_bio(&mut self, region: &RegionCode, new_bio: Option<Bio>) -> Result<bool> {
        self.ensure_region_match(region)?;

        if self.bio == new_bio {
            return Ok(false);
        }

        let old_bio = self.bio.take();
        self.bio = new_bio;

        self.apply_change();
        self.add_event(Box::new(ProfileEvent::BioUpdated {
            id: Self::create_event_id(),
            profile_id: self.id.clone(),
            owner_id: self.owner_id.clone(),
            region: self.region_code.clone(),
            old_bio,
            new_bio:self.bio.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    /// Mise à jour des Medias
    pub fn update_avatar(&mut self, region: &RegionCode, new_avatar_url: Url) -> Result<bool> {
        self.ensure_region_match(region)?;

        if self.avatar_url == Some(new_avatar_url.clone()) {
            return Ok(false);
        }

        let old_avatar_url = std::mem::replace(&mut self.avatar_url, Some(new_avatar_url));
        self.apply_change();

        self.add_event(Box::new(ProfileEvent::AvatarUpdated {
            id: Self::create_event_id(),
            profile_id: self.id.clone(),
            owner_id: self.owner_id.clone(),
            region: self.region_code.clone(),
            old_avatar_url,
            new_avatar_url:  self.avatar_url.clone().unwrap(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn remove_avatar(&mut self, region: &RegionCode) -> Result<bool> {
        self.ensure_region_match(region)?;
        if self.avatar_url.is_none() {
            return Ok(false);
        }

        // let old_avatar_url = self.avatar_url.clone();
        // self.avatar_url = None;
        let old_avatar_url = self.avatar_url.take();
        self.apply_change();

        self.add_event(Box::new(ProfileEvent::AvatarRemoved {
            id: Self::create_event_id(),
            profile_id: self.id.clone(),
            owner_id: self.owner_id.clone(),
            region: self.region_code.clone(),
            old_avatar_url,
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn update_banner(&mut self, region: &RegionCode, new_banner_url: Url) -> Result<bool> {
        self.ensure_region_match(region)?;
        if self.banner_url == Some(new_banner_url.clone()) {
            return Ok(false);
        }

        let old_banner_url = std::mem::replace(&mut self.banner_url, Some(new_banner_url)) ;
        self.apply_change();

        self.add_event(Box::new(ProfileEvent::BannerUpdated {
            id: Self::create_event_id(),
            profile_id: self.id.clone(),
            owner_id: self.owner_id.clone(),
            region: self.region_code.clone(),
            old_banner_url,
            new_banner_url: self.banner_url.clone().unwrap(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn remove_banner(&mut self, region: &RegionCode) -> Result<bool> {
        self.ensure_region_match(region)?;
        if self.banner_url.is_none() {
            return Ok(false);
        }

        let old_banner_url = self.banner_url.take();
        self.apply_change();

        self.add_event(Box::new(ProfileEvent::BannerRemoved {
            id: Self::create_event_id(),
            profile_id: self.id.clone(),
            owner_id: self.owner_id.clone(),
            region: self.region_code.clone(),
            old_banner_url,
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    /// Mise à jour des Metadata
    pub fn update_social_links(&mut self, region: &RegionCode, new_links: Option<SocialLinks>) -> Result<bool> {
        self.ensure_region_match(region)?;

        // 2. Idempotence : Si rien ne change, on ne fait rien
        if self.social_links == new_links {
            return Ok(false);
        }

        // 3. Capturer l'ancien état AVANT la modification
        let old_links = std::mem::replace(&mut self.social_links, new_links);
        self.apply_change();

        // 5. Événement avec les deux états (pour comparaison/audit)
        self.add_event(Box::new(ProfileEvent::SocialLinksUpdated {
            id: Self::create_event_id(),
            profile_id: self.id.clone(),
            owner_id: self.owner_id.clone(),
            region: self.region_code.clone(),
            old_links,
            new_links: self.social_links.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn update_location_label(&mut self, region: &RegionCode, new_label: Option<LocationLabel>) -> Result<bool> {
        self.ensure_region_match(region)?;

        if self.location_label == new_label {
            return Ok(false);
        }

        let old_location = std::mem::replace(&mut self.location_label, new_label);
        self.apply_change();

        // 3. Émission de l'événement
        self.add_event(Box::new(ProfileEvent::LocationLabelUpdated {
            id: Self::create_event_id(),
            profile_id: self.id.clone(),
            owner_id: self.owner_id.clone(),
            region: self.region_code.clone(),
            old_location,
            new_location: self.location_label.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn update_privacy(&mut self, region: &RegionCode, is_private: bool) -> Result<bool> {
        self.ensure_region_match(region)?;

        if self.is_private == is_private {
            return Ok(false);
        }

        self.is_private = is_private;
        self.apply_change();

        self.add_event(Box::new(ProfileEvent::PrivacySettingsChanged {
            id: Self::create_event_id(),
            profile_id: self.id.clone(),
            owner_id: self.owner_id.clone(),
            region: self.region_code.clone(),
            is_private,
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    /// Mise à jour des compteurs internes
    pub fn increment_post_count(&mut self, region: &RegionCode, post_id: PostId) -> Result<bool> {
        self.ensure_region_match(region)?;

        self.post_count.increment();
        self.apply_change();

        // On génère l'événement spécifique avec l'ID du post
        self.add_event(Box::new(ProfileEvent::PostCountIncremented {
            id: Self::create_event_id(),
            profile_id: self.id.clone(),
            owner_id: self.owner_id.clone(),
            region: self.region_code.clone(),
            post_id: post_id.as_uuid(),
            new_count: self.post_count.value(),
            occurred_at: self.updated_at,
        }));

        // Snapshot optionnel tous les 10 posts
        if self.post_count.value() % 10 == 0 {
            self.record_stats_snapshot();
        }

        Ok(true)
    }

    pub fn decrement_post_count(&mut self, region: &RegionCode, post_id: PostId) -> Result<bool> {
        self.ensure_region_match(region)?;

        if self.post_count.value() == 0 {
            return Ok(false);
        }

        self.post_count.decrement();
        self.apply_change();

        // Même logique pour la décrémentation : on utilise le post_id
        // pour s'assurer qu'on ne décrémente pas deux fois si l'appel est rejoué.
        self.add_event(Box::new(ProfileEvent::PostCountDecremented {
            id: Self::create_event_id(),
            profile_id: self.id.clone(),
            owner_id: self.owner_id.clone(),
            region: self.region_code.clone(),
            post_id: post_id.as_uuid(),
            new_count: self.post_count.value(),
            occurred_at: self.updated_at,
        }));

        if self.post_count.value() % 10 == 0 {
            self.record_stats_snapshot();
        }
        Ok(true)
    }

    pub(crate) fn restore_stats(&mut self, stats: ProfileStats) {
        self.stats = stats;
        // On ne touche ni à la version, ni aux événements, ni à updated_at.
    }

    // Helpers
    fn apply_change(&mut self) {
        self.increment_version();
        self.updated_at = Utc::now();
    }

    fn record_stats_snapshot(&mut self) {
        self.add_event(Box::new(ProfileEvent::StatsSnapshotUpdated {
            id: Self::create_event_id(),
            profile_id: self.id.clone(),
            owner_id: self.owner_id.clone(),
            region: self.region_code.clone(),
            follower_count: self.stats.follower_count(),
            following_count: self.stats.following_count(),
            post_count: self.post_count.value(),
            occurred_at: Utc::now(),
        }));
    }

    fn ensure_region_match(&self, region: &RegionCode) -> Result<()> {
        if &self.region_code != region {
            return Err(DomainError::Forbidden {
                reason: "Cross-region operation detected on Profile".into(),
            });
        }
        Ok(())
    }
}

impl EntityMetadata for Profile {
    fn entity_name() -> &'static str {
        "Profile"
    }
}

impl AggregateRoot for Profile {
    fn id(&self) -> String {
        self.id.to_string()
    }
    fn metadata(&self) -> &AggregateMetadata {
        &self.metadata
    }
    fn metadata_mut(&mut self) -> &mut AggregateMetadata {
        &mut self.metadata
    }
}
