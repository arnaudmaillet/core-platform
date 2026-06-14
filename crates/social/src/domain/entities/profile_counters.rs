use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::core::{Entity, LifecycleTracker, ManagedEntity, Result};
use shared_kernel::messaging::{Event, EventEmitter};
use shared_kernel::types::{Counter, ProfileId};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProfileCounters {
    profile_id: ProfileId,
    followers_count: Counter,
    following_count: Counter,
    lifecycle: LifecycleTracker,
}

impl EventEmitter for ProfileCounters {
    fn push_event(&mut self, event: Box<dyn Event>) {
        self.lifecycle.push_event(event);
    }
    fn pull_events(&mut self) -> Vec<Box<dyn Event>> {
        self.lifecycle.pull_events()
    }
}

impl ManagedEntity for ProfileCounters {
    fn lifecycle(&self) -> &LifecycleTracker {
        &self.lifecycle
    }
    fn lifecycle_mut(&mut self) -> &mut LifecycleTracker {
        &mut self.lifecycle
    }
}

impl Entity for ProfileCounters {
    type Id = ProfileId;

    fn entity_name() -> &'static str {
        "ProfileCounters"
    }

    fn map_constraint_to_field(_constraint: &str) -> &'static str {
        "profile_id"
    }

    fn id(&self) -> &Self::Id {
        self.profile_id_as_ref()
    }
    fn updated_at(&self) -> DateTime<Utc> {
        self.lifecycle.updated_at()
    }
}

impl ProfileCounters {
    pub fn new(profile_id: ProfileId) -> Self {
        ProfileCounters {
            profile_id,
            followers_count: Counter::default(),
            following_count: Counter::default(),
            lifecycle: LifecycleTracker::default(),
        }
    }

    pub fn restore(
        profile_id: ProfileId,
        followers_count: Counter,
        following_count: Counter,
        updated_at: DateTime<Utc>,
    ) -> Self {
        ProfileCounters {
            profile_id,
            followers_count,
            following_count,
            lifecycle: LifecycleTracker::restore(updated_at),
        }
    }

    // --- GETTERS ---
    pub fn profile_id(&self) -> ProfileId {
        self.profile_id
    }
    pub(crate) fn profile_id_as_ref(&self) -> &ProfileId {
        &self.profile_id
    }
    pub fn followers_count(&self) -> Counter {
        self.followers_count
    }
    pub fn following_count(&self) -> Counter {
        self.following_count
    }

    // --- MUTATEURS MÉTIERS ---
    pub fn apply_follower_change(&mut self, increment: bool) -> Result<bool> {
        if increment {
            self.followers_count.increment();
        } else {
            self.followers_count.decrement();
        }
        self.lifecycle.record_change();
        Ok(true)
    }

    pub fn apply_following_change(&mut self, increment: bool) -> Result<bool> {
        if increment {
            self.following_count.increment();
        } else {
            self.following_count.decrement();
        }
        self.lifecycle.record_change();
        Ok(true)
    }
}
