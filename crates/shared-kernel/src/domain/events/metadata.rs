// crates/shared-kernel/src/domain/events/metadata.rs

use crate::domain::events::DomainEvent;
use crate::errors::Result;
use serde::{Deserialize, Serialize};

/// Données techniques partagées par tous les agrégats.
/// Évite la duplication de la logique de gestion des événements et de versioning.

#[derive(Debug, Serialize, Deserialize)]
pub struct AggregateMetadata {
    version: u64,
    #[serde(skip)]
    events: Vec<Box<dyn DomainEvent>>,
}

impl AggregateMetadata {
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn version_i64(&self) -> Result<i64> {
        use std::convert::TryInto;
        self.version.try_into().map_err(|_| crate::errors::DomainError::Internal(
            "Version overflow: cannot fit u64 version into i64 database storage".into()
        ))
    }

    /// Crée une nouvelle instance (par défaut version 1 pour une création)
    pub fn new(version: u64) -> Self {
        Self {
            version,
            events: Vec::new(),
        }
    }

    /// RESTAURATION : Utilise ceci dans tes Repositories.
    /// On restaure la version exacte de la DB et on garantit
    /// que la liste d'événements est vide (on ne veut pas re-publier le passé).
    pub fn restore(version: u64) -> Self {
        Self {
            version,
            events: Vec::new(),
        }
    }

    pub fn add_event(&mut self, event: Box<dyn DomainEvent>) {
        self.events.push(event);
    }

    pub fn pull_events(&mut self) -> Vec<Box<dyn DomainEvent>> {
        std::mem::take(&mut self.events)
    }

    pub fn increment_version(&mut self) {
        self.version += 1;
    }
}

impl Default for AggregateMetadata {
    fn default() -> Self {
        Self::new(1)
    }
}

/// Trait maître pour tous les agrégats du système.
/// Grâce aux implémentations par défaut, l'entité n'a qu'à implémenter id() et l'accès aux données.
pub trait AggregateRoot: Send + Sync {
    /// Identifiant unique de l'agrégat sous forme de chaîne
    fn id(&self) -> String;

    /// Accès en lecture seule aux données d'agrégat
    fn metadata(&self) -> &AggregateMetadata;

    /// Accès en écriture aux données d'agrégat
    fn metadata_mut(&mut self) -> &mut AggregateMetadata;

    // --- Implémentations par défaut (Automatiques) ---

    /// Version actuelle de l'agrégat (pour l'Optimistic Concurrency Control)
    fn version(&self) -> u64 {
        self.metadata().version
    }

    fn version_i64(&self) -> Result<i64> {
        self.metadata().version_i64()
    }

    /// Enregistre un fait métier
    fn add_event(&mut self, event: Box<dyn DomainEvent>) {
        self.metadata_mut().add_event(event);
    }

    /// Récupère et vide la file d'attente des événements pour traitement (Outbox)
    fn pull_events(&mut self) -> Vec<Box<dyn DomainEvent>> {
        self.metadata_mut().pull_events()
    }

    /// Incrémente la version technique de l'agrégat
    fn increment_version(&mut self) {
        self.metadata_mut().increment_version();
    }
}

impl Clone for AggregateMetadata {
    fn clone(&self) -> Self {
        Self {
            version: self.version,
            events: Vec::new(),
        }
    }
}

impl TryFrom<i64> for AggregateMetadata {
    type Error = crate::errors::DomainError;

    fn try_from(version: i64) -> Result<Self> {
        if version < 0 {
            return Err(crate::errors::DomainError::Internal(
                "Database returned a negative version number".into()
            ));
        }
        Ok(Self::restore(version as u64))
    }
}
