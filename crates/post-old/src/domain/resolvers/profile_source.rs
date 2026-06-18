// crates/post/src/domain/repositories/profile_source.rs

use async_trait::async_trait;
use shared_kernel::core::Result;
use shared_kernel::types::ProfileId;
use std::collections::{BTreeMap, BTreeSet};

#[async_trait]
pub trait ProfileSource: Send + Sync {
    async fn fetch_from_source(
        &self,
        slugs: &BTreeSet<String>,
    ) -> Result<BTreeMap<String, ProfileId>>;
}
