use async_trait::async_trait;
use post_older::{Post, PostWriteRepositoryStub};
use shared_kernel::types::PostId;

#[async_trait]
pub trait PostRepositoryAsserts {
    async fn assert_post_state<F>(&self, post_id: PostId, check: F)
    where
        F: FnOnce(&Post) + Send;

    async fn assert_not_found(&self, post_id: PostId);
}

#[async_trait]
impl PostRepositoryAsserts for PostWriteRepositoryStub {
    async fn assert_post_state<F>(&self, post_id: PostId, check: F)
    where
        F: FnOnce(&Post) + Send,
    {
        let saved = self
            .get_raw(&post_id)
            .expect("Assertion Failed: Post expected to exist in repository stub memory");

        check(&saved);
    }

    async fn assert_not_found(&self, post_id: PostId) {
        let exists = self.get_raw(&post_id).is_some();

        assert!(
            !exists,
            "Assertion Failed: Expected Post {:?} to be physically non-existent, but it was found",
            post_id
        );
    }
}
