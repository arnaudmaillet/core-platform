// crates/comment/src/domain/repositories/comment.rs

use async_trait::async_trait;
use shared_kernel::core::{PageQuery, PagedResult, Result};
use shared_kernel::types::{PostId};

use crate::entities::Comment;
use crate::types::CommentId;

#[async_trait]
pub trait CommentRepository: Send + Sync {
    /// Sauvegarde un commentaire ou une réponse.
    async fn save(&self, comment: &Comment) -> Result<()>;

    /// Récupère un commentaire racine (Niveau 0) spécifique.
    async fn find_root_by_id(
        &self,
        post_id: PostId,
        comment_id: CommentId,
    ) -> Result<Option<Comment>>;

    /// Récupère une réponse (Niveau 1) spécifique.
    async fn find_reply_by_id(
        &self,
        parent_comment_id: CommentId,
        comment_id: CommentId,
    ) -> Result<Option<Comment>>;

    /// Récupère les commentaires racines (Niveau 0) d'un post triés avec pagination.
    async fn find_roots_by_post(
        &self,
        post_id: PostId,
        query: PageQuery,
    ) -> Result<PagedResult<Comment>>;

    /// Récupère les réponses (Niveau 1) sous un commentaire parent triés avec pagination.
    async fn find_replies_by_parent(
        &self,
        parent_comment_id: CommentId,
        query: PageQuery,
    ) -> Result<PagedResult<Comment>>;

    async fn delete(
        &self,
        post_id: PostId,
        parent_comment_id: Option<CommentId>,
        comment_id: CommentId,
    ) -> Result<()>;
}
