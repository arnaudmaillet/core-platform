// crates/content_comments/src/infrastructure/repositories/scylla_comment_repository.rs

use async_trait::async_trait;
use infra_scylla::scylla::{
    client::session::Session, statement::prepared::PreparedStatement, value::CqlTimestamp,
};
use shared_kernel::core::{Entity, Error, Identifier, PageQuery, PagedResult, Result};
use shared_kernel::types::PostId;
use std::sync::Arc;

use crate::entities::Comment;
use crate::infrastructure::statements::{DELETE_REPLY, DELETE_ROOT, FIND_REPLY_BY_ID, FIND_ROOT_BY_ID, INSERT_REPLY, INSERT_ROOT, SELECT_REPLIES_BY_PARENT, SELECT_ROOTS_BY_POST};
use crate::mappers::{CqlCommentMapper, CqlReplyCommentRow, CqlRootCommentRow};
use crate::repositories::CommentRepository;
use crate::types::CommentId;

pub struct ScyllaCommentStore {
    session: Arc<Session>,
    insert_root_stmt: PreparedStatement,
    insert_reply_stmt: PreparedStatement,
    find_root_stmt: PreparedStatement,
    find_reply_stmt: PreparedStatement,
    select_roots_stmt: PreparedStatement,
    select_replies_stmt: PreparedStatement,
    delete_root_stmt: PreparedStatement,
    delete_reply_stmt: PreparedStatement,
}

impl ScyllaCommentStore {
    pub async fn new(session: Arc<Session>) -> Result<Self> {
        let prepare = |cql: &'static str, sess: Arc<Session>| async move {
            sess.prepare(cql)
                .await
                .map_err(|e| Error::database(format!("ScyllaDB PreparedStatement failed: {}", e)))
        };

        Ok(Self {
            insert_root_stmt: prepare(INSERT_ROOT, session.clone()).await?,
            insert_reply_stmt: prepare(INSERT_REPLY, session.clone()).await?,
            find_root_stmt: prepare(FIND_ROOT_BY_ID, session.clone()).await?,
            find_reply_stmt: prepare(FIND_REPLY_BY_ID, session.clone()).await?,
            select_roots_stmt: prepare(SELECT_ROOTS_BY_POST, session.clone()).await?,
            select_replies_stmt: prepare(SELECT_REPLIES_BY_PARENT, session.clone())
                .await?,
            delete_root_stmt: prepare(DELETE_ROOT, session.clone()).await?,
            delete_reply_stmt: prepare(DELETE_REPLY, session.clone()).await?,
            session,
        })
    }
}

#[async_trait]
impl CommentRepository for ScyllaCommentStore {
    async fn save(&self, comment: &Comment) -> Result<()> {
        let comment_id_uuid = comment.comment_id().as_uuid();
        let post_id_uuid = comment.post_id().as_uuid();
        let profile_id_uuid = comment.profile_id().as_uuid();
        let content_str = comment.content().to_string();

        let edited_at_cql = comment
            .edited_at()
            .map(|dt| CqlTimestamp(dt.timestamp_millis()));
        let updated_at_cql = CqlTimestamp(comment.updated_at().timestamp_millis());

        if let Some(parent_id) = comment.parent_comment_id() {
            let params = (
                parent_id.as_uuid(),
                comment_id_uuid,
                post_id_uuid,
                profile_id_uuid,
                &content_str,
                edited_at_cql,
                updated_at_cql,
            );
            self.session
                .execute_unpaged(&self.insert_reply_stmt, params)
                .await
                .map_err(|e| Error::database(format!("Failed to insert reply: {}", e)))?;
        } else {
            let params = (
                post_id_uuid,
                comment_id_uuid,
                profile_id_uuid,
                &content_str,
                edited_at_cql,
                updated_at_cql,
            );
            self.session
                .execute_unpaged(&self.insert_root_stmt, params)
                .await
                .map_err(|e| Error::database(format!("Failed to insert root comment: {}", e)))?;
        }

        Ok(())
    }

    async fn find_root_by_id(
        &self,
        post_id: PostId,
        comment_id: CommentId,
    ) -> Result<Option<Comment>> {
        let res = self
            .session
            .execute_unpaged(
                &self.find_root_stmt,
                (post_id.as_uuid(), comment_id.as_uuid()),
            )
            .await
            .map_err(|e| Error::database(e.to_string()))?;

        let rows = res
            .into_rows_result()
            .map_err(|e| Error::database(e.to_string()))?;

        if let Some(row) = rows
            .maybe_first_row::<CqlRootCommentRow>()
            .map_err(|e| Error::database(e.to_string()))?
        {
            return Ok(Some(CqlCommentMapper::to_root_domain(row)?));
        }
        Ok(None)
    }

    async fn find_reply_by_id(
        &self,
        parent_comment_id: CommentId,
        comment_id: CommentId,
    ) -> Result<Option<Comment>> {
        let res = self
            .session
            .execute_unpaged(
                &self.find_reply_stmt,
                (parent_comment_id.as_uuid(), comment_id.as_uuid()),
            )
            .await
            .map_err(|e| Error::database(e.to_string()))?;

        let rows = res
            .into_rows_result()
            .map_err(|e| Error::database(e.to_string()))?;

        if let Some(row) = rows
            .maybe_first_row::<CqlReplyCommentRow>()
            .map_err(|e| Error::database(e.to_string()))?
        {
            return Ok(Some(CqlCommentMapper::to_reply_domain(row)?));
        }
        Ok(None)
    }

    async fn find_roots_by_post(
        &self,
        post_id: PostId,
        query: PageQuery,
    ) -> Result<PagedResult<Comment>> {
        let mut stmt = self.select_roots_stmt.clone();
        stmt.set_page_size(query.limit as i32);

        let res = self
            .session
            .execute_unpaged(&stmt, (post_id.as_uuid(),))
            .await
            .map_err(|e| Error::database(e.to_string()))?;

        let rows_res = res
            .into_rows_result()
            .map_err(|e| Error::database(e.to_string()))?;
        let mut rows_iter = rows_res
            .rows::<CqlRootCommentRow>()
            .map_err(|e| Error::database(e.to_string()))?;

        let mut comments = Vec::new();
        while let Some(row_res) = rows_iter.next() {
            let row = row_res.map_err(|e| Error::database(e.to_string()))?;
            comments.push(CqlCommentMapper::to_root_domain(row)?);
        }

        let has_more = comments.len() >= query.limit;
        if has_more {
            comments.truncate(query.limit);
        }

        Ok(PagedResult {
            items: comments,
            next_cursor: if has_more {
                Some("next_page_token".to_string())
            } else {
                None
            },
        })
    }

    async fn find_replies_by_parent(
        &self,
        parent_comment_id: CommentId,
        query: PageQuery,
    ) -> Result<PagedResult<Comment>> {
        let mut stmt = self.select_replies_stmt.clone();
        stmt.set_page_size(query.limit as i32);

        let res = self
            .session
            .execute_unpaged(&stmt, (parent_comment_id.as_uuid(),))
            .await
            .map_err(|e| Error::database(e.to_string()))?;

        let rows_res = res
            .into_rows_result()
            .map_err(|e| Error::database(e.to_string()))?;
        let mut rows_iter = rows_res
            .rows::<CqlReplyCommentRow>()
            .map_err(|e| Error::database(e.to_string()))?;

        let mut replies = Vec::new();
        while let Some(row_res) = rows_iter.next() {
            let row = row_res.map_err(|e| Error::database(e.to_string()))?;
            replies.push(CqlCommentMapper::to_reply_domain(row)?);
        }

        let has_more = replies.len() >= query.limit;
        if has_more {
            replies.truncate(query.limit);
        }

        Ok(PagedResult {
            items: replies,
            next_cursor: if has_more {
                Some("next_page_token".to_string())
            } else {
                None
            },
        })
    }

    async fn delete(
        &self,
        post_id: PostId,
        parent_comment_id: Option<CommentId>,
        comment_id: CommentId,
    ) -> Result<()> {
        if let Some(parent_id) = parent_comment_id {
            self.session
                .execute_unpaged(
                    &self.delete_reply_stmt,
                    (parent_id.as_uuid(), comment_id.as_uuid()),
                )
                .await
                .map_err(|e| Error::database(format!("Failed to delete reply: {}", e)))?;
        } else {
            self.session
                .execute_unpaged(
                    &self.delete_root_stmt,
                    (post_id.as_uuid(), comment_id.as_uuid()),
                )
                .await
                .map_err(|e| Error::database(format!("Failed to delete root comment: {}", e)))?;
        }
        Ok(())
    }
}
