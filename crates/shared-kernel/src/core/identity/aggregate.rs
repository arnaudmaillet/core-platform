// crates/shared-kernel/src/core/identity/aggregate.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    core::Entity,
    messaging::{Event, EventEmitter},
};

pub trait ManagedEntity: Entity + EventEmitter + Send + Sync {
    fn lifecycle(&self) -> &LifecycleTracker;
    fn lifecycle_mut(&mut self) -> &mut LifecycleTracker;
}

/// Métadonnées fondamentales partagées par TOUS les agrégats (SQL et NoSQL).
#[derive(Debug, Serialize, Deserialize)]
pub struct LifecycleTracker {
    updated_at: DateTime<Utc>,
    #[serde(skip)]
    events: Vec<Box<dyn Event>>,
}

impl LifecycleTracker {
    pub fn new() -> Self {
        Self {
            updated_at: Utc::now(),
            events: Vec::new(),
        }
    }

    pub fn restore(updated_at: DateTime<Utc>) -> Self {
        Self {
            updated_at,
            events: Vec::new(),
        }
    }

    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    pub fn events(&self) -> &[Box<dyn Event>] {
        &self.events
    }

    pub fn is_events_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn push_event(&mut self, event: Box<dyn Event>) {
        self.events.push(event);
    }

    pub fn pull_events(&mut self) -> Vec<Box<dyn Event>> {
        std::mem::take(&mut self.events)
    }

    pub fn record_change(&mut self) {
        self.updated_at = Utc::now();
    }
}

impl Default for LifecycleTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for LifecycleTracker {
    fn clone(&self) -> Self {
        Self {
            updated_at: self.updated_at,
            events: Vec::new(),
        }
    }
}

impl From<DateTime<Utc>> for LifecycleTracker {
    fn from(updated_at: DateTime<Utc>) -> Self {
        Self::restore(updated_at)
    }
}
