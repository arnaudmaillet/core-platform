// crates/content_comments/src/application/context/command.rs

use shared_kernel::{
    command::CommandTarget,
    core::{Error, Result},
    messaging::EventEmitter,
    types::{PostId, ProfileId},
};
use uuid::Uuid;

use crate::application::context::CommentAppContext;
use crate::entities::Comment;
use crate::types::CommentId;

#[derive(Clone)]
pub struct CommentCommandContext {
    app: CommentAppContext,
    operator_id: ProfileId,
}

impl CommentCommandContext {
    pub fn new(app: CommentAppContext, operator_id: ProfileId) -> Self {
        Self { app, operator_id }
    }

    pub fn app(&self) -> &CommentAppContext {
        &self.app
    }
    pub fn operator_id(&self) -> ProfileId {
        self.operator_id
    }

    pub async fn ensure_executable(&self, command_id: Uuid) -> Result<bool> {
        let exists = self
            .app
            .idempotency_repo()
            .exists(None, &command_id)
            .await?;
        Ok(!exists)
    }

    pub async fn save_idempotency(&self, command_id: Uuid) -> Result<()> {
        self.app.idempotency_repo().save(None, &command_id).await?;
        Ok(())
    }

    pub async fn fetch_verified(
        &self,
        target: &CommandTarget<CommentId>,
        post_id: PostId,
        parent_comment_id: Option<CommentId>,
    ) -> Result<Comment> {
        let comment = if let Some(parent_id) = parent_comment_id {
            self.app
                .comment_repo()
                .find_reply_by_id(parent_id, target.id)
                .await?
        } else {
            self.app
                .comment_repo()
                .find_root_by_id(post_id, target.id)
                .await?
        };

        let comment = comment.ok_or_else(|| Error::not_found("Comment", target.id.to_string()))?;

        Ok(comment)
    }

    pub async fn save(&self, comment: &mut Comment, command_id: Option<Uuid>) -> Result<()> {
        if comment.profile_id() != self.operator_id {
            return Err(Error::forbidden(&format!(
                "Profil {} non autorisé à modifier ce commentaire",
                self.operator_id
            )));
        }

        let _events = comment.pull_events();

        self.app.comment_repo().save(comment).await?;

        if let Some(cmd_id) = command_id {
            self.save_idempotency(cmd_id).await?;
        }
        Ok(())
    }
}
