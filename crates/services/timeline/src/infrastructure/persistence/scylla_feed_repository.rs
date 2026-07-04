use std::sync::Arc;

use async_trait::async_trait;
use scylla::observability::history::HistoryListener;
use scylla::statement::unprepared::Statement;
use scylla::value::CqlTimestamp;
use scylla_storage::{ProfileKind as ScyllaProfileKind, ScyllaClient, ScyllaStorageError};
use crate::application::port::FeedRepository;
use crate::domain::aggregate::FeedEntry;
use crate::domain::value_object::{AuthorId, PostId, ProfileId};
use crate::error::TimelineError;
use crate::infrastructure::persistence::model::FeedItemRow;

fn scylla_err(e: scylla::errors::ExecutionError) -> TimelineError {
    TimelineError::Scylla(ScyllaStorageError::from(e))
}

fn row_err(ctx: &'static str, e: impl ToString) -> TimelineError {
    TimelineError::DomainViolation {
        field:   ctx.to_owned(),
        message: e.to_string(),
    }
}

pub struct ScyllaFeedRepository {
    client: Arc<ScyllaClient>,
}

impl ScyllaFeedRepository {
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
impl FeedRepository for ScyllaFeedRepository {
    async fn insert(
        &self,
        profile_id: &ProfileId,
        entry:      &FeedEntry,
    ) -> Result<(), TimelineError> {
        let stmt = self.strict_stmt(
            "INSERT INTO timeline.feed_items_by_profile \
             (profile_id, published_at, post_id, author_id) \
             VALUES (?, ?, ?, ?)",
        );
        self.client
            .session
            .execute_unpaged(
                stmt,
                (
                    profile_id.as_uuid(),
                    CqlTimestamp(entry.published_at_ms),
                    entry.post_id.as_uuid(),
                    entry.author_id.as_uuid(),
                ),
            )
            .await
            .map_err(scylla_err)?;
        Ok(())
    }

    async fn insert_batch(
        &self,
        profile_id: &ProfileId,
        entries:    &[FeedEntry],
    ) -> Result<(), TimelineError> {
        for entry in entries {
            self.insert(profile_id, entry).await?;
        }
        Ok(())
    }

    async fn list_recent(
        &self,
        profile_id: &ProfileId,
        before_ms:  i64,
        limit:      i32,
    ) -> Result<Vec<FeedEntry>, TimelineError> {
        const COLS: &str = "profile_id, published_at, post_id, author_id";

        let rows: Vec<FeedItemRow> = if before_ms == i64::MAX {
            let stmt = self.fast_stmt(&format!(
                "SELECT {COLS} FROM timeline.feed_items_by_profile \
                 WHERE profile_id = ? LIMIT ?"
            ));
            self.client
                .session
                .execute_unpaged(stmt, (profile_id.as_uuid(), limit))
                .await
                .map_err(scylla_err)?
                .into_rows_result()
                .map_err(|e| row_err("result", e))?
                .rows::<FeedItemRow>()
                .map_err(|e| row_err("deserialize", e))?
                .filter_map(|r| r.ok())
                .collect()
        } else {
            let stmt = self.fast_stmt(&format!(
                "SELECT {COLS} FROM timeline.feed_items_by_profile \
                 WHERE profile_id = ? \
                   AND published_at < ? \
                 LIMIT ?"
            ));
            self.client
                .session
                .execute_unpaged(
                    stmt,
                    (profile_id.as_uuid(), CqlTimestamp(before_ms), limit),
                )
                .await
                .map_err(scylla_err)?
                .into_rows_result()
                .map_err(|e| row_err("result", e))?
                .rows::<FeedItemRow>()
                .map_err(|e| row_err("deserialize", e))?
                .filter_map(|r| r.ok())
                .collect()
        };

        rows.into_iter()
            .map(|r| row_to_entry(&r))
            .collect()
    }

    async fn delete(
        &self,
        profile_id: &ProfileId,
        post_id:    &PostId,
        published_at_ms: i64,
    ) -> Result<(), TimelineError> {
        let stmt = self.strict_stmt(
            "DELETE FROM timeline.feed_items_by_profile \
             WHERE profile_id = ? AND published_at = ? AND post_id = ?",
        );
        self.client
            .session
            .execute_unpaged(
                stmt,
                (
                    profile_id.as_uuid(),
                    CqlTimestamp(published_at_ms),
                    post_id.as_uuid(),
                ),
            )
            .await
            .map_err(scylla_err)?;
        Ok(())
    }

    async fn list_by_author(
        &self,
        profile_id: &ProfileId,
        author_id:  &AuthorId,
    ) -> Result<Vec<(PostId, i64)>, TimelineError> {
        // Full-partition scan filtered by author_id in application code.
        // Safe because partitions are per-follower (bounded by feed_cap).
        // ALLOW FILTERING is deliberately avoided — the partition scan is
        // bounded and ScyllaDB is efficient at full-partition reads.
        const COLS: &str = "profile_id, published_at, post_id, author_id";
        let stmt = self.fast_stmt(&format!(
            "SELECT {COLS} FROM timeline.feed_items_by_profile \
             WHERE profile_id = ?"
        ));

        let target_author = author_id.as_uuid();

        let rows: Vec<FeedItemRow> = self
            .client
            .session
            .execute_unpaged(stmt, (profile_id.as_uuid(),))
            .await
            .map_err(scylla_err)?
            .into_rows_result()
            .map_err(|e| row_err("result", e))?
            .rows::<FeedItemRow>()
            .map_err(|e| row_err("deserialize", e))?
            .filter_map(|r| r.ok())
            .collect();

        let filtered = rows
            .into_iter()
            .filter(|r| r.author_id == target_author)
            .map(|r| (PostId::from_uuid(r.post_id), r.published_at.0))
            .collect::<Vec<_>>();

        Ok(filtered)
    }
}

fn row_to_entry(r: &FeedItemRow) -> Result<FeedEntry, TimelineError> {
    let post_id   = PostId::from_uuid(r.post_id);
    let author_id = AuthorId::from_uuid(r.author_id);
    Ok(FeedEntry::new(post_id, author_id, r.published_at.0))
}
