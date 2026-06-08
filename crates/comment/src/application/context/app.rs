// crates/content_comments/src/application/context/app.rs

use crate::application::context::{CommentCommandContext, CommentQueryContext};
use crate::repositories::CommentRepository;
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::types::ProfileId;
use std::sync::Arc;

#[derive(Clone)]
pub struct CommentAppContext {
    comment_repo: Arc<dyn CommentRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
}

impl CommentAppContext {
    pub fn new(
        comment_repo: Arc<dyn CommentRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
    ) -> Self {
        Self {
            comment_repo,
            idempotency_repo,
        }
    }

    pub fn query(&self) -> CommentQueryContext {
        CommentQueryContext::new(self.clone())
    }

    pub fn command(&self, operator_id: ProfileId) -> CommentCommandContext {
        CommentCommandContext::new(self.clone(), operator_id)
    }

    pub fn comment_repo(&self) -> Arc<dyn CommentRepository> {
        self.comment_repo.clone()
    }

    pub fn idempotency_repo(&self) -> Arc<dyn IdempotencyRepository> {
        self.idempotency_repo.clone()
    }
}
