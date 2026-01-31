// crates/profile/src/domain/entities/profile.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::__rt::sleep;
use uuid::Uuid;
use shared_kernel::domain::events::{AggregateRoot, AggregateMetadata};
use shared_kernel::domain::entities::EntityMetadata;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::value_objects::{Counter, LocationLabel, RegionCode, Url, AccountId, Username, PostId};

use crate::domain::builders::ProfileBuilder;
use crate::domain::events::ProfileEvent;
use crate::domain::value_objects::{Bio, DisplayName, ProfileStats, SocialLinks};


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Profile {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub display_name: DisplayName,
    pub username: Username,
    pub bio: Option<Bio>,
    pub avatar_url: Option<Url>,
    pub banner_url: Option<Url>,
    pub location_label: Option<LocationLabel>,
    pub social_links: Option<SocialLinks>,
    pub stats: ProfileStats,
    pub post_count: Counter,
    pub is_private: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: AggregateMetadata,
}

impl Profile {
    pub fn new_initial(
        account_id: AccountId,
        region: RegionCode,
        display_name: DisplayName,
        username: Username
    ) -> Self {
        let occurred_at = Utc::now();
        let mut profile = ProfileBuilder::new(
            account_id.clone(),
            region.clone(),
            display_name.clone(),
            username.clone()
        ).build();

        profile.add_event(Box::new(ProfileEvent::ProfileCreated {
            id: Self::create_event_id(),
            account_id,
            region,
            display_name,
            username,
            occurred_at,
        }));

        profile
    }

    fn create_event_id() -> Uuid {
        Uuid::now_v7()
    }

    /// Mise à jour identity - Action Critique
    pub fn update_username(&mut self, new_username: Username) -> bool {
        if self.username == new_username {
            return false;
        }

        let old_username = self.username.clone();
        self.username = new_username.clone();

        self.apply_change();
        self.add_event(Box::new(ProfileEvent::UsernameChanged {
            id: Self::create_event_id(),
            account_id: self.account_id.clone(),
            old_username,
            new_username,
            occurred_at: self.updated_at,
        }));

        true
    }

    pub fn update_display_name(&mut self, new_display_name: DisplayName) -> bool {
        if self.display_name == new_display_name{
            return false;
        }

        let old_display_name = self.display_name.clone();
        self.display_name = new_display_name.clone();

        self.apply_change();
        self.add_event(Box::new(ProfileEvent::DisplayNameChanged {
            id: Self::create_event_id(),
            account_id: self.account_id.clone(),
            old_display_name,
            new_display_name,
            occurred_at: self.updated_at,
        }));

        true
    }

    pub fn update_bio(&mut self, new_bio: Option<Bio>) -> bool {
        if self.bio == new_bio {
            return false;
        }

        let old_bio = self.bio.take();
        self.bio = new_bio.clone();

        self.apply_change();
        self.add_event(Box::new(ProfileEvent::BioUpdated {
            id: Self::create_event_id(),
            account_id: self.account_id.clone(),
            old_bio,
            new_bio,
            occurred_at: self.updated_at,
        }));

        true
    }

    /// Mise à jour des Medias
    pub fn update_avatar(&mut self, new_avatar_url: Url) -> bool {
        if self.avatar_url == Some(new_avatar_url.clone()) {
            return false;
        }

        let old_avatar_url = self.avatar_url.take();
        self.avatar_url = Some(new_avatar_url.clone());
        self.apply_change();

        self.add_event(Box::new(ProfileEvent::AvatarUpdated {
            id: Self::create_event_id(),
            account_id: self.account_id.clone(),
            old_avatar_url,
            new_avatar_url,
            occurred_at: self.updated_at,
        }));
        true
    }

    pub fn remove_avatar(&mut self) -> bool {
        if self.avatar_url.is_none() {
            return false;
        }

        let old_avatar_url = self.avatar_url.clone();
        self.avatar_url = None;
        self.apply_change();

        self.add_event(Box::new(ProfileEvent::AvatarRemoved {
            id: Self::create_event_id(),
            account_id: self.account_id.clone(),
            old_avatar_url,
            occurred_at: self.updated_at,
        }));

        true
    }

    pub fn update_banner(&mut self, new_banner_url: Url) -> bool {
        if self.banner_url == Some(new_banner_url.clone()) {
            return false;
        }

        let old_banner_url = self.banner_url.take();
        self.banner_url = Some(new_banner_url.clone());
        self.apply_change();

        self.add_event(Box::new(ProfileEvent::BannerUpdated {
            id: Self::create_event_id(),
            account_id: self.account_id.clone(),
            old_banner_url,
            new_banner_url,
            occurred_at: self.updated_at,
        }));

        true
    }

    pub fn remove_banner(&mut self) -> bool {
        if self.banner_url.is_none() {
            return false;
        }

        let old_banner_url = self.avatar_url.clone();
        self.avatar_url = None;
        self.apply_change();

        self.add_event(Box::new(ProfileEvent::BannerRemoved {
            id: Self::create_event_id(),
            account_id: self.account_id.clone(),
            old_banner_url,
            occurred_at: self.updated_at,
        }));

        true
    }

    /// Mise à jour des Metadata
    pub fn update_social_links(&mut self, new_links: Option<SocialLinks>) -> bool {
        // 1. On nettoie l'objet (si c'est un objet plein de "None", il devient "None")
        let normalized = new_links.and_then(|l| l.simplify());

        // 2. Idempotence : Si rien ne change, on ne fait rien
        if self.social_links == normalized {
            return false;
        }

        // 3. Capturer l'ancien état AVANT la modification
        let old_links = self.social_links.clone();

        // 4. Mutation
        self.social_links = normalized;
        self.apply_change();

        // 5. Événement avec les deux états (pour comparaison/audit)
        self.add_event(Box::new(ProfileEvent::SocialLinksUpdated {
            id: Self::create_event_id(),
            account_id: self.account_id.clone(),
            old_links,
            new_links: self.social_links.clone(), // Le nouvel état nettoyé
            occurred_at: self.updated_at,
        }));

        true
    }

    pub fn update_location_label(&mut self, new_label: Option<LocationLabel>) -> bool {
        if self.location_label == new_label {
            return false;
        }

        let old_location = self.location_label.clone();
        self.location_label = new_label;
        self.apply_change();

        // 3. Émission de l'événement
        self.add_event(Box::new(ProfileEvent::LocationLabelUpdated {
            id: Self::create_event_id(),
            account_id: self.account_id.clone(),
            old_location,
            new_location: self.location_label.clone(),
            occurred_at: self.updated_at,
        }));

        true
    }

    pub fn update_privacy(&mut self, is_private: bool) -> bool {
        if self.is_private == is_private {
            return false;
        }

        self.is_private = is_private;
        self.apply_change();

        self.add_event(Box::new(ProfileEvent::PrivacySettingsChanged {
            id: Self::create_event_id(),
            account_id: self.account_id.clone(),
            region: self.region_code.clone(),
            is_private,
            occurred_at: self.updated_at,
        }));

        true
    }

    /// Mise à jour des compteurs internes
    pub fn increment_post_count(&mut self, post_id: PostId) -> bool {
        self.post_count.increment();
        self.apply_change();

        // On génère l'événement spécifique avec l'ID du post
        self.add_event(Box::new(ProfileEvent::PostCountIncremented {
            id: Self::create_event_id(),
            account_id: self.account_id.clone(),
            post_id: post_id.as_uuid(),
            new_count: self.post_count.value(),
            occurred_at: self.updated_at,
        }));

        // Snapshot optionnel tous les 10 posts
        if self.post_count.value() % 10 == 0 {
            self.record_stats_snapshot();
        }

        true
    }

    pub fn decrement_post_count(&mut self, post_id: PostId) -> bool {
        if self.post_count.value() == 0 {
            return  false;
        }

        self.post_count.decrement();
        self.apply_change();

        // Même logique pour la décrémentation : on utilise le post_id
        // pour s'assurer qu'on ne décrémente pas deux fois si l'appel est rejoué.
        self.add_event(Box::new(ProfileEvent::PostCountDecremented {
            id: Self::create_event_id(),
            account_id: self.account_id.clone(),
            post_id: post_id.as_uuid(),
            new_count: self.post_count.value(),
            occurred_at: self.updated_at,
        }));

        if self.post_count.value() % 10 == 0 {
            self.record_stats_snapshot();
        }
        true
    }


    // Helpers
    fn apply_change(&mut self) {
        self.increment_version();
        self.updated_at = Utc::now();
    }

    fn record_stats_snapshot(&mut self) {
        self.add_event(Box::new(ProfileEvent::StatsSnapshotUpdated {
            id: Self::create_event_id(),
            account_id: self.account_id.clone(),
            follower_count: self.stats.follower_count.value(),
            following_count: self.stats.following_count.value(),
            post_count: self.post_count.value(),
            occurred_at: Utc::now(),
        }));
    }
}

impl EntityMetadata for Profile {
    fn entity_name() -> &'static str { "Profile" }
}

impl AggregateRoot for Profile {
    fn id(&self) -> String { self.account_id.as_string() }
    fn metadata(&self) -> &AggregateMetadata { &self.metadata }
    fn metadata_mut(&mut self) -> &mut AggregateMetadata { &mut self.metadata }
}