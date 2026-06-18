// crates/post/src/domain/repositories/profile_resolver.rs

use async_trait::async_trait;
use shared_kernel::core::Result;
use shared_kernel::types::ProfileId;
use std::collections::{BTreeMap, BTreeSet};

#[async_trait]
pub trait ProfileResolver: Send + Sync {
    async fn resolve_slugs(&self, slugs: &BTreeSet<String>) -> Result<BTreeMap<String, ProfileId>>;
}
