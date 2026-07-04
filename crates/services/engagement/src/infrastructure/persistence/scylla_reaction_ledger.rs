use std::sync::Arc;

use async_trait::async_trait;
use scylla::observability::history::HistoryListener;
use scylla::statement::unprepared::Statement;
use scylla::value::CqlTimestamp;
use scylla_storage::{ProfileKind as ScyllaProfileKind, ScyllaClient, ScyllaStorageError};
use uuid::Uuid;

use crate::application::port::ReactionLedger;
use crate::domain::value_object::{PostId, ProfileId, ReactionKind};
use crate::error::EngagementError;
use crate::infrastructure::persistence::model::ReactionRow;

fn scylla_err(e: scylla::errors::ExecutionError) -> EngagementError {
    EngagementError::Scylla(ScyllaStorageError::from(e))
}

fn row_err(ctx: &'static str, e: impl ToString) -> EngagementError {
    EngagementError::DomainViolation {
        field:   ctx.to_owned(),
        message: e.to_string(),
    }
}

pub struct ScyllaReactionLedger {
    client: Arc<ScyllaClient>,
}

impl ScyllaReactionLedger {
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
impl ReactionLedger for ScyllaReactionLedger {
    async fn upsert(
        &self,
        post_id:     &PostId,
        profile_id:  &ProfileId,
        kind:        ReactionKind,
        weight:      i64,
        event_at_ms: i64,
    ) -> Result<(), EngagementError> {
        let stmt = self.strict_stmt(
            "INSERT INTO engagement.post_reactions \
             (post_id, profile_id, kind, weight, reacted_at) \
             VALUES (?, ?, ?, ?, ?)",
        );
        self.client
            .session
            .execute_unpaged(
                stmt,
                (
                    post_id.as_uuid(),
                    profile_id.as_uuid(),
                    kind.as_tinyint(),
                    weight as i32,
                    CqlTimestamp(event_at_ms),
                ),
            )
            .await
            .map_err(scylla_err)?;

        Ok(())
    }

    async fn remove(
        &self,
        post_id:    &PostId,
        profile_id: &ProfileId,
    ) -> Result<(), EngagementError> {
        let stmt = self.strict_stmt(
            "DELETE FROM engagement.post_reactions \
             WHERE post_id = ? AND profile_id = ?",
        );
        self.client
            .session
            .execute_unpaged(stmt, (post_id.as_uuid(), profile_id.as_uuid()))
            .await
            .map_err(scylla_err)?;

        Ok(())
    }

    async fn scan_for_recovery(
        &self,
        post_id: &PostId,
    ) -> Result<Vec<ReactionRow>, EngagementError> {
        let stmt = self.fast_stmt(
            "SELECT post_id, profile_id, kind, weight, reacted_at \
             FROM engagement.post_reactions \
             WHERE post_id = ?",
        );
        let rows = self.client
            .session
            .execute_unpaged(stmt, (post_id.as_uuid(),))
            .await
            .map_err(scylla_err)?
            .into_rows_result()
            .map_err(|e| row_err("scan_for_recovery:rows", e))?
            .rows::<ReactionRow>()
            .map_err(|e| row_err("scan_for_recovery:iter", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| row_err("scan_for_recovery:deser", e))?;

        Ok(rows)
    }

    async fn apply_interaction_delta(
        &self,
        post_id:       &PostId,
        view_delta:    i64,
        share_delta:   i64,
        comment_delta: i64,
    ) -> Result<(), EngagementError> {
        let post_uuid: Uuid = post_id.as_uuid();

        if view_delta != 0 {
            let stmt = self.strict_stmt(
                "UPDATE engagement.post_interaction_counters \
                 SET view_count = view_count + ? \
                 WHERE post_id = ?",
            );
            self.client
                .session
                .execute_unpaged(stmt, (view_delta, post_uuid))
                .await
                .map_err(scylla_err)?;
        }

        if share_delta != 0 {
            let stmt = self.strict_stmt(
                "UPDATE engagement.post_interaction_counters \
                 SET share_count = share_count + ? \
                 WHERE post_id = ?",
            );
            self.client
                .session
                .execute_unpaged(stmt, (share_delta, post_uuid))
                .await
                .map_err(scylla_err)?;
        }

        if comment_delta != 0 {
            let stmt = self.strict_stmt(
                "UPDATE engagement.post_interaction_counters \
                 SET comment_count = comment_count + ? \
                 WHERE post_id = ?",
            );
            self.client
                .session
                .execute_unpaged(stmt, (comment_delta, post_uuid))
                .await
                .map_err(scylla_err)?;
        }

        Ok(())
    }
}
