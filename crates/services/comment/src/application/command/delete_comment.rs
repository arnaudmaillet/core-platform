use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::{
    application::port::{CommentEventPublisher, CommentRepository},
    domain::{
        aggregate::DeletionStrategy,
        value_object::{CommentId, ProfileId},
    },
    error::CommentError,
};

pub struct DeleteCommentCommand {
    pub comment_id: String,
    pub author_id:  String,
}

impl Command for DeleteCommentCommand {}

impl Validate for DeleteCommentCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.comment_id.trim().is_empty() {
            v.push(FieldViolation::new("comment_id", "CMT-VAL-001", "comment_id must not be empty"));
        }
        if self.author_id.trim().is_empty() {
            v.push(FieldViolation::new("author_id", "CMT-VAL-003", "author_id must not be empty"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct DeleteCommentHandler<R, P> {
    pub repository: Arc<R>,
    pub publisher:  Arc<P>,
}

impl<R, P> CommandHandler<DeleteCommentCommand> for DeleteCommentHandler<R, P>
where
    R: CommentRepository,
    P: CommentEventPublisher,
{
    type Error = CommentError;

    async fn handle(&self, envelope: Envelope<DeleteCommentCommand>) -> Result<(), CommentError> {
        let cmd = &envelope.payload;

        let comment_id = CommentId::try_from(cmd.comment_id.as_str())?;
        let caller_id  = ProfileId::try_from(cmd.author_id.as_str())?;

        let mut comment = self.repository.find_by_id(&comment_id).await?
            .ok_or_else(|| CommentError::CommentNotFound {
                comment_id: comment_id.as_str(),
            })?;

        if comment.author_id().as_uuid() != caller_id.as_uuid() {
            return Err(CommentError::AuthorMismatch {
                comment_id: comment_id.as_str(),
                caller_id:  caller_id.as_str(),
            });
        }

        let has_replies = self.repository
            .has_active_replies(comment.post_id(), &comment_id)
            .await?;

        let strategy = comment.delete(has_replies)?;

        match strategy {
            DeletionStrategy::Tombstone => {
                self.repository.soft_delete(&comment).await?;
                tracing::debug!(
                    comment_id = %comment_id,
                    "comment tombstoned (has active replies)"
                );
            }
            DeletionStrategy::Purge => {
                self.repository.purge(&comment).await?;
                tracing::debug!(
                    comment_id = %comment_id,
                    "comment purged (leaf node)"
                );
            }
        }

        for event in comment.take_events() {
            self.publisher.publish(&event).await?;
        }

        Ok(())
    }
}
