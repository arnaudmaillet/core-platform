use crate::resolvers::{ProfileResolver, ProfileSource};
use async_trait::async_trait;
use shared_kernel::cache::{CacheRepository, CacheRepositoryExt};
use shared_kernel::core::Result;
use shared_kernel::types::ProfileId;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

pub struct CachedProfileResolver {
    cache_repo: Arc<dyn CacheRepository>,
    fallback_source: Arc<dyn ProfileSource>,
}

impl CachedProfileResolver {
    pub fn new(
        cache_repo: Arc<dyn CacheRepository>,
        fallback_source: Arc<dyn ProfileSource>,
    ) -> Self {
        Self {
            cache_repo,
            fallback_source,
        }
    }
}

#[async_trait]
impl ProfileResolver for CachedProfileResolver {
    async fn resolve_slugs(&self, slugs: &BTreeSet<String>) -> Result<BTreeMap<String, ProfileId>> {
        let mut results = BTreeMap::new();
        let mut missing = BTreeSet::new();

        for slug in slugs {
            let key = format!("profile:slug:{}", slug);
            if let Ok(Some(id)) = self.cache_repo.get_obj::<ProfileId>(&key).await {
                results.insert(slug.clone(), id);
            } else {
                missing.insert(slug.clone());
            }
        }

        if !missing.is_empty() {
            let fresh_data = self.fallback_source.fetch_from_source(&missing).await?;

            let mut entries = Vec::new();
            for (slug, id) in &fresh_data {
                if let Ok(json) = serde_json::to_string(id) {
                    entries.push((format!("profile:slug:{}", slug), json));
                }
            }

            let entries_ref: Vec<(&str, String)> = entries
                .iter()
                .map(|(k, v)| (k.as_str(), v.clone()))
                .collect();

            let _ = self
                .cache_repo
                .set_many(entries_ref, Some(Duration::from_secs(3600)))
                .await;

            results.extend(fresh_data);
        }

        Ok(results)
    }
}
