// crates/account/src/infrastructure/sharding/models.rs

use crate::cache::CacheRepository;
use crate::types::RegionCode;
use std::sync::Arc;

#[derive(Clone)]
pub struct ShardNode {
    pub region: RegionCode,
    pub shard_id: u16,
    pub storage: Arc<ShardStorage>,
}

pub struct ShardStorage {
    // Le pool SQL (Optionnel si un shard n'est que NoSQL)
    pub postgres: Option<sqlx::PgPool>,
    // Le cache (Redis)
    pub redis: Arc<dyn CacheRepository>,
    // Si tu ajoutes du NoSQL plus tard :
    // pub mongo: Option<mongodb::Client>,
}
