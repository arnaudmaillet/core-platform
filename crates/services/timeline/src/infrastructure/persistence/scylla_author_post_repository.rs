use std::sync::Arc;

use async_trait::async_trait;
use scylla::observability::history::HistoryListener;
use scylla::statement::unprepared::Statement;
use scylla::value::CqlTimestamp;
use scylla_storage::{ProfileKind as ScyllaProfileKind, ScyllaClient, ScyllaStorageError};

use crate::application::port::AuthorPostRepository;
use crate::domain::aggregate::FeedEntry;
use crate::domain::value_object::{AuthorId, AuthorTier, PostId};
use crate::error::TimelineError;
use crate::infrastructure::persistence::model::AuthorPostRow;

fn scylla_err(e: scylla::errors::ExecutionError) -> TimelineError {
    TimelineError::Scylla(ScyllaStorageError::from(e))
}

fn row_err(ctx: &'static str, e: impl ToString) -> TimelineError {
    TimelineError::DomainViolation {
        field:   ctx.to_owned(),
        message: e.to_string(),
    }
}

pub struct ScyllaAuthorPostRepository {
    client: Arc<ScyllaClient>,
}

impl ScyllaAuthorPostRepository {
    pub fn new(client: Arc<ScyllaClient>) -> Self {
        Self { client }
    }

    fn strict_stmt(&self, cql: &str) -> Statement {
        let mut s = Statement::new(cql);
        s.set_execution_profile_handle(Some(
            self.client
                .profiles
                .get(ScyllaProfileKind::Strict)
                .clone()
                .into_handle_with_label("strict".to_string()),
        ));
        s.set_history_listener(
            Arc::clone(&self.client.history_listener) as Arc<dyn HistoryListener>,
        );
        s
    }

    fn fast_stmt(&self, cql: &str) -> Statement {
        let mut s = Statement::new(cql);
        s.set_execution_profile_handle(Some(
            self.client
                .profiles
                .get(ScyllaProfileKind::Fast)
                .clone()
                .into_handle_with_label("fast".to_string()),
        ));
        s.set_history_listener(
            Arc::clone(&self.client.history_listener) as Arc<dyn HistoryListener>,
        );
        s
    }
}

#[async_trait]
impl AuthorPostRepository for ScyllaAuthorPostRepository {
    async fn insert(
        &self,
        author_id:  &AuthorId,
        post_id:    &PostId,
        tier:       AuthorTier,
        published_at_ms: i64,
    ) -> Result<(), TimelineError> {
        let stmt = self.strict_stmt(
            "INSERT INTO timeline.posts_by_author \
             (author_id, published_at, post_id, author_tier) \
             VALUES (?, ?, ?, ?)",
        );
        self.client
            .session
            .execute_unpaged(
                stmt,
                (
                    author_id.as_uuid(),
                    CqlTimestamp(published_at_ms),
                    post_id.as_uuid(),
                    tier.as_i8(),
                ),
            )
            .await
            .map_err(scylla_err)?;
        Ok(())
    }

    async fn delete(
        &self,
        author_id:  &AuthorId,
        post_id:    &PostId,
        published_at_ms: i64,
    ) -> Result<(), TimelineError> {
        let stmt = self.strict_stmt(
            "DELETE FROM timeline.posts_by_author \
             WHERE author_id = ? AND published_at = ? AND post_id = ?",
        );
        self.client
            .session
            .execute_unpaged(
                stmt,
                (
                    author_id.as_uuid(),
                    CqlTimestamp(published_at_ms),
                    post_id.as_uuid(),
                ),
            )
            .await
            .map_err(scylla_err)?;
        Ok(())
    }

    async fn list_recent(
        &self,
        author_id: &AuthorId,
        before_ms: i64,
        limit:     i32,
    ) -> Result<Vec<FeedEntry>, TimelineError> {
        const COLS: &str = "author_id, published_at, post_id, author_tier";

        let rows: Vec<AuthorPostRow> = if before_ms == i64::MAX {
            let stmt = self.fast_stmt(&format!(
                "SELECT {COLS} FROM timeline.posts_by_author \
                 WHERE author_id = ? LIMIT ?"
            ));
            self.client
                .session
                .execute_unpaged(stmt, (author_id.as_uuid(), limit))
                .await
                .map_err(scylla_err)?
                .into_rows_result()
                .map_err(|e| row_err("result", e))?
                .rows::<AuthorPostRow>()
                .map_err(|e| row_err("deserialize", e))?
                .filter_map(|r| r.ok())
                .collect()
        } else {
            let stmt = self.fast_stmt(&format!(
                "SELECT {COLS} FROM timeline.posts_by_author \
                 WHERE author_id = ? \
                   AND published_at < ? \
                 LIMIT ?"
            ));
            self.client
                .session
                .execute_unpaged(
                    stmt,
                    (author_id.as_uuid(), CqlTimestamp(before_ms), limit),
                )
                .await
                .map_err(scylla_err)?
                .into_rows_result()
                .map_err(|e| row_err("result", e))?
                .rows::<AuthorPostRow>()
                .map_err(|e| row_err("deserialize", e))?
                .filter_map(|r| r.ok())
                .collect()
        };

        rows.into_iter()
            .map(|r| {
                let post_id   = PostId::from_uuid(r.post_id);
                let author_id = AuthorId::from_uuid(r.author_id);
                Ok::<_, TimelineError>(FeedEntry::new(post_id, author_id, r.published_at.0))
            })
            .collect()
    }
}
