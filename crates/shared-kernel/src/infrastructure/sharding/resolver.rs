// crates/account/src/infrastructure/sharding/resolver.rs

use std::collections::HashMap;
use crate::domain::Identifier;
use crate::core::InfrastructureError;
use crate::infrastructure::sharding::ShardNode;
use crate::domain::value_objects::{AccountId, RegionCode};

pub struct ShardResolver {
    // On indexe par région, et chaque région a un vecteur de nodes (shards)
    nodes_by_region: HashMap<RegionCode, Vec<ShardNode>>,
}

impl ShardResolver {
    pub fn new(nodes: Vec<ShardNode>) -> Self {
        let mut nodes_by_region: HashMap<RegionCode, Vec<ShardNode>> = HashMap::new();
        
        for node in nodes {
            nodes_by_region
                .entry(node.region.clone())
                .or_default()
                .push(node);
        }
        
        // Optionnel : Trier les nodes par shard_id pour assurer la stabilité du modulo
        for nodes in nodes_by_region.values_mut() {
            nodes.sort_by_key(|n| n.shard_id);
        }

        Self { nodes_by_region }
    }

    /// La méthode "Magique" : Géo + Modulo
    pub fn resolve(&self, account_id: &AccountId, region: &RegionCode) -> Result<&ShardNode, InfrastructureError> {
        // 1. On cherche les shards de la région
        let region_shards = self.nodes_by_region.get(region)
            .ok_or_else(|| InfrastructureError::UnsupportedRegion(region.to_string()))?;

        // 2. On vérifie que la région n'est pas une coquille vide
        if region_shards.is_empty() {
            return Err(InfrastructureError::EmptyShardPool(region.to_string()));
        }

        // 3. LOGIQUE DU MODULO
        // L'UUID est distribué uniformément, donc id % len est parfait
        let id_value = account_id.as_uuid().as_u128();
        let index = (id_value % region_shards.len() as u128) as usize;

        Ok(&region_shards[index])
    }
}