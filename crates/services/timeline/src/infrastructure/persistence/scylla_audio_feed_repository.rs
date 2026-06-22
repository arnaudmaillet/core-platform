use std::sync::Arc;

use scylla::observability::history::HistoryListener;
use scylla::statement::unprepared::Statement;
use scylla::value::CqlTimestamp;
use scylla_storage::{ProfileKind as ScyllaProfileKind, ScyllaClient, ScyllaStorageError};

use crate::application::port::{AudioFeedRepository, AudioFeedRow as PortAudioFeedRow};
use crate::domain::value_object::{AudioId, AuthorId, PostId};
use crate::error::TimelineError;
use crate::infrastructure::persistence::model::AudioFeedRow;

fn scylla_err(e: scylla::errors::ExecutionError) -> TimelineError {
    TimelineError::Scylla(ScyllaStorageError::from(e))
}

fn row_err(ctx: &'static str, e: impl ToString) -> TimelineError {
    TimelineError::DomainViolation {
        field:   ctx.to_owned(),
        message: e.to_string(),
    }
}

pub struct ScyllaAudioFeedRepository {
    client: Arc<ScyllaClient>,
}

impl ScyllaAudioFeedRepository {
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

impl AudioFeedRepository for ScyllaAudioFeedRepository {
    async fn insert(
        &self,
        audio_id:        &AudioId,
        post_id:         &PostId,
        author_id:       &AuthorId,
        published_at_ms: i64,
    ) -> Result<(), TimelineError> {
        let stmt = self.strict_stmt(
            "INSERT INTO timeline.posts_by_audio \
             (audio_id, published_at, post_id, author_id) \
             VALUES (?, ?, ?, ?) IF NOT EXISTS",
        );
        self.client
            .session
            .execute_unpaged(
                stmt,
                (
                    audio_id.as_uuid(),
                    CqlTimestamp(published_at_ms),
                    post_id.as_uuid(),
                    author_id.as_uuid(),
                ),
            )
            .await
            .map_err(|e| {
                tracing::warn!(
                    audio_id = %audio_id,
                    post_id  = %post_id,
                    error    = %e,
                    "TML-7001"
                );
                scylla_err(e)
            })?;
        Ok(())
    }

    async fn delete(
        &self,
        audio_id:        &AudioId,
        post_id:         &PostId,
        published_at_ms: i64,
    ) -> Result<(), TimelineError> {
        let stmt = self.strict_stmt(
            "DELETE FROM timeline.posts_by_audio \
             WHERE audio_id = ? AND published_at = ? AND post_id = ?",
        );
        self.client
            .session
            .execute_unpaged(
                stmt,
                (
                    audio_id.as_uuid(),
                    CqlTimestamp(published_at_ms),
                    post_id.as_uuid(),
                ),
            )
            .await
            .map_err(|e| {
                tracing::warn!(
                    audio_id = %audio_id,
                    post_id  = %post_id,
                    error    = %e,
                    "TML-7002"
                );
                scylla_err(e)
            })?;
        Ok(())
    }

    async fn list(
        &self,
        audio_id:  &AudioId,
        before_ms: Option<i64>,
        limit:     i32,
    ) -> Result<Vec<PortAudioFeedRow>, TimelineError> {
        const COLS: &str = "audio_id, published_at, post_id, author_id";

        let effective_before = before_ms.unwrap_or(i64::MAX);

        let rows: Vec<AudioFeedRow> = if effective_before == i64::MAX {
            let stmt = self.fast_stmt(&format!(
                "SELECT {COLS} FROM timeline.posts_by_audio \
                 WHERE audio_id = ? LIMIT ?"
            ));
            self.client
                .session
                .execute_unpaged(stmt, (audio_id.as_uuid(), limit))
                .await
                .map_err(scylla_err)?
                .into_rows_result()
                .map_err(|e| row_err("result", e))?
                .rows::<AudioFeedRow>()
                .map_err(|e| row_err("deserialize", e))?
                .filter_map(|r| r.ok())
                .collect()
        } else {
            let stmt = self.fast_stmt(&format!(
                "SELECT {COLS} FROM timeline.posts_by_audio \
                 WHERE audio_id = ? AND published_at < ? LIMIT ?"
            ));
            self.client
                .session
                .execute_unpaged(
                    stmt,
                    (audio_id.as_uuid(), CqlTimestamp(effective_before), limit),
                )
                .await
                .map_err(scylla_err)?
                .into_rows_result()
                .map_err(|e| row_err("result", e))?
                .rows::<AudioFeedRow>()
                .map_err(|e| row_err("deserialize", e))?
                .filter_map(|r| r.ok())
                .collect()
        };

        rows.into_iter()
            .map(|r| {
                let post_id   = PostId::from_uuid(r.post_id);
                let author_id = AuthorId::from_uuid(r.author_id);
                Ok(PortAudioFeedRow {
                    post_id,
                    author_id,
                    published_at_ms: r.published_at.0,
                })
            })
            .collect()
    }
}
