use async_trait::async_trait;
use geo_discovery::resolvers::EngagementResolver;
use shared_kernel::{core::Result, types::PostId};
use std::collections::HashMap;

pub struct EngagementResolverStub;

#[async_trait]
impl EngagementResolver for EngagementResolverStub {
    async fn resolve_scores(&self, post_ids: &[PostId]) -> Result<HashMap<PostId, f64>> {
        let mut scores = HashMap::with_capacity(post_ids.len());
        for id in post_ids {
            scores.insert(*id, 1.0);
        }
        Ok(scores)
    }
}
