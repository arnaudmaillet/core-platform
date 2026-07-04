//! Redis key builders. Single-key today, but the asset id is hash-tagged so any
//! future multi-key script stays slot-safe on Redis Cluster.

use crate::domain::value_object::AssetId;

/// Cached deliverable view of an asset: `media:asset:{<id>}`.
pub fn asset_key(id: &AssetId) -> String {
    format!("media:asset:{{{id}}}")
}
