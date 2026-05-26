use async_trait::async_trait;
use post::resolvers::ProfileResolver;
use shared_kernel::{core::Result, types::ProfileId};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::RwLock;

#[derive(Default)]
pub struct ProfileResolverStub {
    pub mappings: RwLock<BTreeMap<String, ProfileId>>,
}

impl ProfileResolverStub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_stub_map(&self, mappings: BTreeMap<String, ProfileId>) {
        let mut w = self.mappings.write().unwrap();
        *w = mappings;
    }
}

#[async_trait]
impl ProfileResolver for ProfileResolverStub {
    async fn resolve_slugs(&self, slugs: &BTreeSet<String>) -> Result<BTreeMap<String, ProfileId>> {
        let mappings = self.mappings.read().unwrap();
        let mut resolved = BTreeMap::new();
        for slug in slugs {
            if let Some(id) = mappings.get(slug) {
                resolved.insert(slug.clone(), *id);
            }
        }
        Ok(resolved)
    }
}
