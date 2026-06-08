// crates/content_comments/src/application/dtos/mod.rs

use crate::entities::Comment;
use crate::types::CommentUserProfile;

#[derive(Debug, Clone)]
pub struct CommentWithProfile {
    pub comment: Comment,
    pub profile: Option<CommentUserProfile>,
}
