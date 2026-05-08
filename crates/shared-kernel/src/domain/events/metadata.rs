// crates/shared-kernel/src/domain/events/metadata.rs

use crate::domain::events::DomainEvent;
use crate::errors::{DomainError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Conteneur de données techniques partagées.
/// Gère l'état interne du versioning et de la file d'attente des événements.
#[derive(Debug, Serialize, Deserialize)]
pub struct AggregateMetadata {
    version: u64,
    updated_at: DateTime<Utc>,
    #[serde(skip)]
    events: Vec<Box<dyn DomainEvent>>,
}

impl AggregateMetadata {
    pub const INITIAL_VERSION: u64 = 0;

    /// Initialisation pour une nouvelle entité (création)
    pub fn new(version: u64) -> Self {
        Self {
            version,
            updated_at: Utc::now(),
            events: Vec::new(),
        }
    }

    /// RESTAURATION : À utiliser par les Repositories lors du chargement depuis la DB.
    /// Garantit que la liste d'événements est vide pour éviter de re-publier le passé.
    pub fn restore(version: u64, updated_at: DateTime<Utc>) -> Self {
        Self {
            version,
            updated_at,
            events: Vec::new(),
        }
    }

    // --- ACCESSEURS ---

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    /// Helper pour le stockage en base de données (souvent i64)
    pub fn version_i64(&self) -> Result<i64> {
        use std::convert::TryInto;
        self.version.try_into().map_err(|_| {
            DomainError::Internal(
                "Version overflow: cannot fit u64 version into i64 database storage".into(),
            )
        })
    }

    // --- MUTATEURS ---

    /// Ajoute un événement à la file d'attente (Outbox pattern)
    pub fn push_event(&mut self, event: Box<dyn DomainEvent>) {
        self.events.push(event);
    }

    /// Récupère et vide la file d'attente des événements
    pub fn pull_events(&mut self) -> Vec<Box<dyn DomainEvent>> {
        std::mem::take(&mut self.events)
    }

    /// Incrémente la version et met à jour l'horodatage
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
            updated_at: self.updated_at,
            // On ne clone JAMAIS les événements en attente
            events: Vec::new(),
        }
    }
}

/// Conversion pratique depuis un tuple venant de la base de données
impl TryFrom<(i64, DateTime<Utc>)> for AggregateMetadata {
    type Error = DomainError;

    fn try_from(value: (i64, DateTime<Utc>)) -> Result<Self> {
        let (version, updated_at) = value;
        if version < 0 {
            return Err(DomainError::Internal(
                "Database returned a negative version number".into(),
            ));
        }
        Ok(Self::restore(version as u64, updated_at))
    }
}
