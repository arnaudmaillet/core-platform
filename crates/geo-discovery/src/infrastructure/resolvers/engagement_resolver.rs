// crates/geo_discovery/src/infrastructure/resolvers/mock_engagement_resolver.rs

use async_trait::async_trait;
use std::collections::HashMap;
use shared_kernel::core::Result;
use shared_kernel::types::PostId;
use crate::resolvers::EngagementResolver;

pub struct MockEngagementResolver;

#[async_trait]
impl EngagementResolver for MockEngagementResolver {
    async fn resolve_scores(&self, post_ids: &[PostId]) -> Result<HashMap<PostId, f64>> {
        let mut mock_scores = HashMap::new();
        for id in post_ids {
            // Temporairement, on donne un score par défaut de 0.0 à tout le monde
            mock_scores.insert(*id, 0.0);
        }
        Ok(mock_scores)
    }
}