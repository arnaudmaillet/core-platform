// Dans crates/shared-kernel/src/application/sharding/model.rs

use crate::types::Region;

/// Identifiant unique et opaque d'un Shard au sein du cluster.
/// Dérivé avec Eq et Hash pour servir de clé de routage dans l'infrastructure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShardNode {
    pub region: Region,
    pub shard_id: u16,
}
