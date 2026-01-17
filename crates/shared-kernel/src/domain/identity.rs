// crates/shared-kernel/src/domain/identity.rs

use uuid::Uuid;

/// Trait pour uniformiser les IDs à travers le système.
/// Permet de passer de l'UUID à l'ULID si nécessaire sans casser le domaine.
pub trait Identifier: serde::Serialize + for<'de> serde::Deserialize<'de> + Clone + Send + Sync + PartialEq {
    fn to_uuid(&self) -> Uuid;
    fn to_string(&self) -> String;
}