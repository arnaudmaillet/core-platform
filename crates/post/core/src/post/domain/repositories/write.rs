use crate::Post;
use async_trait::async_trait;
use shared_kernel::core::Result;
use shared_kernel::types::{PostId, ProfileId};

#[async_trait]
pub trait PostWriteRepository: Send + Sync {
    async fn save(&self, post: &Post) -> Result<()>;
    async fn delete(&self, post_id: &PostId, author_id: &ProfileId) -> Result<()>;
}
