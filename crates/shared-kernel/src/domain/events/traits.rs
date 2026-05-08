// crates/shared-kernel/src/domain/events/traits.rs

use crate::domain::{
    entities::Versioned,
    events::{DomainEvent, metadata::AggregateMetadata},
};

/// Capacité d'un objet à produire des événements métier.
pub trait EventEmitter {
    fn push_event(&mut self, event: Box<dyn DomainEvent>);
    fn pull_events(&mut self) -> Vec<Box<dyn DomainEvent>>;
}

/// Trait maître pour tous les agrégats du système.
/// Un agrégat est un objet Versionné qui émet des événements et possède un ID unique.
pub trait AggregateRoot: Versioned + EventEmitter + Send + Sync {
    /// Identifiant unique de l'agrégat sous forme de chaîne (pour l'infra/le stockage)
    fn id(&self) -> String;

    /// Accès aux métadonnées techniques (nécessaire pour les implémentations par défaut)
    fn metadata(&self) -> &AggregateMetadata;

    /// Accès mutable aux métadonnées techniques
    fn metadata_mut(&mut self) -> &mut AggregateMetadata;
}
