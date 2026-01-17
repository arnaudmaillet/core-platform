// crates/profile/src/domain/entities/profile.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::domain::events::{AggregateRoot, AggregateMetadata};
use shared_kernel::domain::entities::EntityMetadata;
use shared_kernel::domain::value_objects::{Counter, LocationLabel, RegionCode, Url, AccountId, Username, PostId};
use shared_kernel::errors::{DomainError, Result};

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
    pub social_links: SocialLinks,
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
        let mut profile = ProfileBuilder::new(
            account_id.clone(),
            region.clone(),
            display_name.clone(),
            username.clone()
        ).build();

        profile.add_event(Box::new(ProfileEvent::ProfileCreated {
            account_id: account_id,
            region,
            display_name: display_name.as_str().to_string(),
            username: username.as_str().to_string(),
            occurred_at: Utc::now(),
        }));

        profile
    }

    /// Mise à jour du pseudo (Slug) - Action Critique
    pub fn update_username(&mut self, new_username: Username) -> bool {
        if self.username == new_username {
            return false;
        }

        let old_username = self.username.as_str().to_string();
        self.username = new_username.clone();

        self.apply_change();
        self.add_event(Box::new(ProfileEvent::UsernameChanged {
            account_id: self.account_id.clone(),
            old_username,
            new_username: new_username.as_str().to_string(),
            occurred_at: self.updated_at,
        }));

        true
    }

    /// Mise à jour combinée Metadata (Bio, DisplayName, Location)
    pub fn update_metadata(&mut self, name: DisplayName, bio: Option<Bio>, location: Option<LocationLabel>) -> bool {
        if self.display_name == name && self.bio == bio && self.location_label == location {
            return false;
        }
        self.display_name = name.clone();
        self.bio = bio.clone();
        self.location_label = location.clone();

        self.apply_change();
        self.add_event(Box::new(ProfileEvent::MetadataUpdated {
            account_id: self.account_id.clone(),
            display_name: name.as_str().to_string(),
            bio: bio.map(|b| b.as_str().to_string()),
            location,
            occurred_at: self.updated_at,
        }));
        true
    }

    /// Mise à jour des URLs de médias
    pub fn update_avatar(&mut self, url: Option<Url>) -> bool {
        if self.avatar_url == url {
            return false;
        }

        self.avatar_url = url.clone();
        self.apply_change();

        self.add_event(Box::new(ProfileEvent::MediaUpdated {
            account_id: self.account_id.clone(),
            avatar_url: url,
            banner_url: self.banner_url.clone(),
            occurred_at: self.updated_at,
        }));
        true
    }

    /// Change la bannière
    pub fn update_banner(&mut self, url: Option<Url>) -> bool {
        if self.banner_url == url {
            return false;
        }

        self.banner_url = url.clone();
        self.apply_change();

        self.add_event(Box::new(ProfileEvent::MediaUpdated {
            account_id: self.account_id.clone(),
            avatar_url: self.avatar_url.clone(),
            banner_url: url,
            occurred_at: self.updated_at,
        }));

        true
    }

    /// Mise à jour des liens sociaux
    pub fn update_social_links(&mut self, links: SocialLinks) {
        self.social_links = links.clone();

        self.apply_change();
        self.add_event(Box::new(ProfileEvent::SocialLinksUpdated {
            account_id: self.account_id.clone(),
            links,
            occurred_at: self.updated_at,
        }));
    }

    pub fn update_privacy(&mut self, is_private: bool) -> bool {
        if self.is_private == is_private {
            return false;
        }

        self.is_private = is_private;
        self.apply_change();

        self.add_event(Box::new(ProfileEvent::PrivacySettingsChanged {
            account_id: self.account_id.clone(),
            region: self.region_code.clone(),
            is_private,
            occurred_at: self.updated_at,
        }));

        true
    }

    pub fn increment_post_count(&mut self, post_id: PostId) {
        self.post_count.increment();
        self.apply_change();

        // On génère l'événement spécifique avec l'ID du post
        self.add_event(Box::new(ProfileEvent::PostCountIncremented {
            account_id: self.account_id.clone(),
            post_id: post_id.as_uuid(),
            new_count: self.post_count.value(),
            occurred_at: self.updated_at,
        }));

        // Snapshot optionnel tous les 10 posts
        if self.post_count.value() % 10 == 0 {
            self.record_stats_snapshot();
        }
    }

    pub fn decrement_post_count(&mut self, post_id: PostId) -> Result<()> {
        if self.post_count.value() == 0 {
            return Err(DomainError::Validation {
                field: "post_count",
                reason: "Cannot decrement a counter that is already at zero".to_string(),
            });
        }

        self.post_count.decrement();
        self.apply_change();

        // Même logique pour la décrémentation : on utilise le post_id
        // pour s'assurer qu'on ne décrémente pas deux fois si l'appel est rejoué.
        self.add_event(Box::new(ProfileEvent::PostCountDecremented {
            account_id: self.account_id.clone(),
            post_id: post_id.as_uuid(),
            new_count: self.post_count.value(),
            occurred_at: self.updated_at,
        }));

        if self.post_count.value() % 10 == 0 {
            self.record_stats_snapshot();
        }

        Ok(())
    }


    // Helpers
    fn apply_change(&mut self) {
        self.increment_version();
        self.updated_at = Utc::now();
    }

    fn record_stats_snapshot(&mut self) {
        self.add_event(Box::new(ProfileEvent::StatsSnapshotUpdated {
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
    fn id(&self) -> String { self.account_id.to_string() }
    fn metadata(&self) -> &AggregateMetadata { &self.metadata }
    fn metadata_mut(&mut self) -> &mut AggregateMetadata { &mut self.metadata }
}