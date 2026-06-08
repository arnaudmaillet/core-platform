// crates/content_comments/src/infrastructure/mappers/cql_comment.rs

use chrono::{DateTime, Utc};
use infra_scylla::scylla;
use infra_scylla::scylla::value::CqlTimestamp;
use infra_scylla::scylla_macros::DeserializeRow;
use shared_kernel::core::{Error, Identifier, LifecycleTracker, Result};
use shared_kernel::types::{PostId, ProfileId};
use uuid::Uuid;

use crate::entities::Comment;
use crate::types::{CommentContent, CommentId};

#[derive(Debug, DeserializeRow)]
pub struct CqlRootCommentRow {
    pub post_id: Uuid,
    pub comment_id: Uuid,
    pub profile_id: Uuid,
    pub content: String,
    pub edited_at: Option<CqlTimestamp>,
    pub updated_at: CqlTimestamp,
}

#[derive(Debug, DeserializeRow)]
pub struct CqlReplyCommentRow {
    pub parent_comment_id: Uuid,
    pub comment_id: Uuid,
    pub post_id: Uuid,
    pub profile_id: Uuid,
    pub content: String,
    pub edited_at: Option<CqlTimestamp>,
    pub updated_at: CqlTimestamp,
}

pub struct CqlCommentMapper;

impl CqlCommentMapper {
    /// Map une ligne de commentaire de niveau 0
    pub fn to_root_domain(row: CqlRootCommentRow) -> Result<Comment> {
        let comment_id = CommentId::from_uuid(row.comment_id);
        let created_at = extract_timestamp_from_uuid_v7(row.comment_id)?;

        let edited_at = row
            .edited_at
            .and_then(|cql| DateTime::from_timestamp_millis(cql.0));
        let updated_at = DateTime::from_timestamp_millis(row.updated_at.0)
            .ok_or_else(|| Error::internal("Invalid technical updated_at from database"))?;

        Ok(Comment::restore(
            comment_id,
            PostId::from_uuid(row.post_id),
            ProfileId::from_uuid(row.profile_id),
            None,
            CommentContent::try_new(row.content)?,
            created_at,
            edited_at,
            LifecycleTracker::restore(updated_at),
        ))
    }

    /// Map une ligne de réponse de niveau 1
    pub fn to_reply_domain(row: CqlReplyCommentRow) -> Result<Comment> {
        let comment_id = CommentId::from_uuid(row.comment_id);
        let created_at = extract_timestamp_from_uuid_v7(row.comment_id)?;

        let edited_at = row
            .edited_at
            .and_then(|cql| DateTime::from_timestamp_millis(cql.0));
        let updated_at = DateTime::from_timestamp_millis(row.updated_at.0)
            .ok_or_else(|| Error::internal("Invalid technical updated_at from database"))?;

        Ok(Comment::restore(
            comment_id,
            PostId::from_uuid(row.post_id),
            ProfileId::from_uuid(row.profile_id),
            Some(CommentId::from_uuid(row.parent_comment_id)),
            CommentContent::try_new(row.content)?,
            created_at,
            edited_at,
            LifecycleTracker::restore(updated_at),
        ))
    }
}

fn extract_timestamp_from_uuid_v7(uuid: Uuid) -> Result<DateTime<Utc>> {
    let uuid_bytes = uuid.as_bytes();
    let mut ts_millis: u64 = 0;
    for i in 0..6 {
        ts_millis = (ts_millis << 8) | (uuid_bytes[i] as u64);
    }

    DateTime::from_timestamp_millis(ts_millis as i64)
        .ok_or_else(|| Error::internal("Failed to parse creation timestamp from Comment UUIDv7"))
}
