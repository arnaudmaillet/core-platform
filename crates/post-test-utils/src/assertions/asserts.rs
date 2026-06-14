// crates/post/src/application/utils/assertions.rs

use async_trait::async_trait;
use post::entities::Post;
use shared_kernel::types::PostId;

use crate::stores::PostStoreStub;

#[async_trait]
pub trait PostRepositoryAsserts {
    async fn assert_post_state<F>(&self, post_id: PostId, check: F)
    where
        F: FnOnce(&Post) + Send;

    async fn assert_not_found(&self, post_id: PostId);
}

#[async_trait]
impl PostRepositoryAsserts for PostStoreStub {
    async fn assert_post_state<F>(&self, post_id: PostId, check: F)
    where
        F: FnOnce(&Post) + Send,
    {
        let saved = self
            .find_direct(post_id)
            .await
            .expect("Assertion Failed: Post expected to exist in repository stub memory");

        check(&saved);
    }

    async fn assert_not_found(&self, post_id: PostId) {
        let exists = self.find_direct(post_id).await.is_ok();

        assert!(
            !exists,
            "Assertion Failed: Expected Post {:?} to be physically non-existent, but it was found",
            post_id
        );
    }
}
