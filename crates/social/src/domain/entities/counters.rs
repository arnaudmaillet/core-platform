use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::core::{AggregateMetadata, AggregateRoot, Entity, Result, Versioned};
use shared_kernel::messaging::{Event, EventEmitter};
use shared_kernel::types::{Counter, ProfileId};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProfileCounters {
    profile_id: ProfileId,
    followers_count: Counter,
    following_count: Counter,
    metadata: AggregateMetadata,
}

impl Versioned for ProfileCounters {
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

impl EventEmitter for ProfileCounters {
    fn push_event(&mut self, event: Box<dyn Event>) {
        self.metadata.push_event(event);
    }
    fn pull_events(&mut self) -> Vec<Box<dyn Event>> {
        self.metadata.pull_events()
    }
}

impl AggregateRoot for ProfileCounters {
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

impl Entity for ProfileCounters {
    type Id = ProfileId;

    fn entity_name() -> &'static str {
        "ProfileCounters"
    }

    fn map_constraint_to_field(constraint: &str) -> &'static str {
        match constraint {
            "profile_counters_pkey" => "profile_id",
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

impl ProfileCounters {
    /// Initialise les compteurs par défaut à zéro pour un nouveau profil
    pub fn new(profile_id: ProfileId) -> Self {
        ProfileCounters {
            profile_id,
            followers_count: Counter::default(),
            following_count: Counter::default(),
            metadata: AggregateMetadata::default(),
        }
    }

    /// Reconstitue l'état de l'agrégat depuis l'infrastructure (ScyllaDB / Redis)
    pub fn restore(
        profile_id: ProfileId,
        followers_count: Counter,
        following_count: Counter,
        version: u64,
        updated_at: DateTime<Utc>,
    ) -> Self {
        ProfileCounters {
            profile_id,
            followers_count,
            following_count,
            metadata: AggregateMetadata::restore(version, updated_at),
        }
    }

    // --- GETTERS ---

    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }
    pub fn followers_count(&self) -> Counter {
        self.followers_count
    }
    pub fn following_count(&self) -> Counter {
        self.following_count
    }

    // --- MUTATEURS MÉTIERS ---

    /// Applique une variation sur le compteur de followers
    pub fn apply_follower_change(&mut self, increment: bool) -> Result<bool> {
        if increment {
            self.followers_count.increment();
        } else {
            self.followers_count.decrement();
        }
        self.record_change();
        Ok(true)
    }

    /// Applique une variation sur le compteur de following
    pub fn apply_following_change(&mut self, increment: bool) -> Result<bool> {
        if increment {
            self.following_count.increment();
        } else {
            self.following_count.decrement();
        }
        self.record_change();
        Ok(true)
    }
}
