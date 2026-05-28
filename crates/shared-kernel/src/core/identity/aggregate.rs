use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    core::{Error, Result, Versioned},
    messaging::{Event, EventEmitter},
};

pub trait AggregateRoot: Versioned + EventEmitter + Send + Sync {
    fn id(&self) -> String;
    fn metadata(&self) -> &AggregateMetadata;
    fn metadata_mut(&mut self) -> &mut AggregateMetadata;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AggregateMetadata {
    version: u64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    #[serde(skip)]
    events: Vec<Box<dyn Event>>,
}

impl AggregateMetadata {
    pub const INITIAL_VERSION: u64 = 0;

    pub fn new(version: u64) -> Self {
        let now = Utc::now();
        Self {
            version,
            created_at: now,
            updated_at: now,
            events: Vec::new(),
        }
    }

    pub fn restore(version: u64, created_at: DateTime<Utc>, updated_at: DateTime<Utc>) -> Self {
        Self {
            version,
            created_at,
            updated_at,
            events: Vec::new(),
        }
    }

    // --- ACCESSEURS ---

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    pub fn is_events_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn version_i64(&self) -> Result<i64> {
        use std::convert::TryInto;
        self.version.try_into().map_err(|_| {
            Error::internal("Version overflow: cannot fit u64 version into i64 database storage")
        })
    }

    // --- MUTATEURS ---

    pub fn push_event(&mut self, event: Box<dyn Event>) {
        self.events.push(event);
    }

    pub fn pull_events(&mut self) -> Vec<Box<dyn Event>> {
        std::mem::take(&mut self.events)
    }

    pub fn record_change(&mut self) {
        self.version += 1;
        self.updated_at = Utc::now();
    }
}

// --- IMPLÉMENTATIONS DE TRAITS STANDARDS ---

impl Default for AggregateMetadata {
    fn default() -> Self {
        Self::new(Self::INITIAL_VERSION)
    }
}

impl Clone for AggregateMetadata {
    fn clone(&self) -> Self {
        Self {
            version: self.version,
            created_at: self.created_at,
            updated_at: self.updated_at,
            events: Vec::new(),
        }
    }
}

impl TryFrom<(i64, DateTime<Utc>, DateTime<Utc>)> for AggregateMetadata {
    type Error = Error;

    fn try_from(value: (i64, DateTime<Utc>, DateTime<Utc>)) -> Result<Self> {
        let (version, created_at, updated_at) = value;
        if version < 0 {
            return Err(Error::internal(format!(
                "Database returned a negative version number: {}",
                version
            )));
        }
        Ok(Self::restore(version as u64, created_at, updated_at))
    }
}
