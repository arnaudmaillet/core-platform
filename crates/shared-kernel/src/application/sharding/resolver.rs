// crates/shared-kernel/src/application/sharding/resolver.rs

use crate::core::{Error, Identifier};
use crate::sharding::ShardNode;
use crate::types::{AccountId, Region};
use std::collections::HashMap;

pub struct ShardResolver {
    // On indexe par région, et chaque région a sa liste de descripteurs de shards
    nodes_by_region: HashMap<Region, Vec<ShardNode>>,
}

impl ShardResolver {
    pub fn new(nodes: Vec<ShardNode>) -> Self {
        let mut nodes_by_region: HashMap<Region, Vec<ShardNode>> = HashMap::new();

        for node in nodes {
            nodes_by_region.entry(node.region).or_default().push(node);
        }

        // Tri des nœuds par shard_id pour garantir la stabilité du calcul de modulo
        for shards in nodes_by_region.values_mut() {
            shards.sort_by_key(|n| n.shard_id);
        }

        Self { nodes_by_region }
    }

    /// Détermine quel ShardNode doit héberger ou traiter les données de cet AccountId
    pub fn resolve(&self, account_id: &AccountId, region: &Region) -> Result<ShardNode, Error> {
        // 1. On cherche les shards configurés pour cette région
        let region_shards = self.nodes_by_region.get(region).ok_or_else(|| {
            Error::precondition_failed(format!(
                "Region '{}' is not supported by this cluster",
                region
            ))
        })?;

        // 2. Sécurité : On s'assure qu'on a au moins un nœud cible
        if region_shards.is_empty() {
            return Err(Error::internal(format!(
                "No shards available for region {}",
                region
            )));
        }

        // 3. LOGIQUE DU MODULO
        // L'UUID étant distribué uniformément, l'indexation par modulo est parfaitement équilibrée
        let id_value = account_id.as_uuid().as_u128();
        let index = (id_value % region_shards.len() as u128) as usize;

        // On retourne une copie (Copy) du ShardNode plutôt qu'une référence,
        // c'est plus simple à manipuler et léger (Region + u16).
        Ok(region_shards[index])
    }
}
