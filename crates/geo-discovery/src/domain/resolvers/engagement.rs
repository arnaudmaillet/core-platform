// crates/geo_discovery/src/domain/resolvers/engagement_resolver.rs

use async_trait::async_trait;
use std::collections::HashMap;
use shared_kernel::core::Result;
use shared_kernel::types::PostId;

#[async_trait]
pub trait EngagementResolver: Send + Sync {
    /// Récupère les scores de viralité actuels d'un lot de posts (Batch)
    /// pour permettre la réhydratation ou la reconstruction des index chauds.
    async fn resolve_scores(&self, post_ids: &[PostId]) -> Result<HashMap<PostId, f64>>;
}