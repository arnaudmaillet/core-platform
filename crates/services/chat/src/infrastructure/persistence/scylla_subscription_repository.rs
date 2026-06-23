use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use scylla_storage::ScyllaClient;
use uuid::Uuid;

use crate::application::port::SubscriptionRepository;
use crate::domain::value_object::{ConversationId, ProfileId};
use crate::error::ChatError;
use crate::infrastructure::persistence::bucket::subscription_bucket;
use crate::infrastructure::persistence::statement::{analytical, fast, row_err, scylla_err, strict};
use crate::infrastructure::persistence::time::to_cql;

/// ScyllaDB adapter for the Audience Plane subscription set.
///
/// Maintains the two denormalized tables in lockstep:
/// `subscriptions_by_conversation` (hash-bucketed) and `subscriptions_by_user`
/// (the reverse index). Writes are idempotent upserts; the dual write is
/// best-effort consistent.
pub struct ScyllaSubscriptionRepository {
    client: Arc<ScyllaClient>,
}

impl ScyllaSubscriptionRepository {
    pub fn new(client: Arc<ScyllaClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl SubscriptionRepository for ScyllaSubscriptionRepository {
    async fn subscribe(
        &self,
        conversation_id: &ConversationId,
        subscriber_id:   &ProfileId,
    ) -> Result<(), ChatError> {
        let now    = to_cql(Utc::now());
        let bucket = subscription_bucket(subscriber_id.as_uuid());

        let by_conv = strict(
            &self.client,
            "INSERT INTO chat.subscriptions_by_conversation \
             (conversation_id, bucket, subscriber_id, subscribed_at) \
             VALUES (?, ?, ?, ?)",
        );
        self.client
            .session
            .execute_unpaged(
                by_conv,
                (conversation_id.as_uuid(), bucket, subscriber_id.as_uuid(), now),
            )
            .await
            .map_err(scylla_err)?;

        let by_user = strict(
            &self.client,
            "INSERT INTO chat.subscriptions_by_user \
             (subscriber_id, conversation_id, subscribed_at) \
             VALUES (?, ?, ?)",
        );
        self.client
            .session
            .execute_unpaged(
                by_user,
                (subscriber_id.as_uuid(), conversation_id.as_uuid(), now),
            )
            .await
            .map_err(scylla_err)?;

        Ok(())
    }

    async fn unsubscribe(
        &self,
        conversation_id: &ConversationId,
        subscriber_id:   &ProfileId,
    ) -> Result<(), ChatError> {
        let bucket = subscription_bucket(subscriber_id.as_uuid());

        let by_conv = strict(
            &self.client,
            "DELETE FROM chat.subscriptions_by_conversation \
             WHERE conversation_id = ? AND bucket = ? AND subscriber_id = ?",
        );
        self.client
            .session
            .execute_unpaged(
                by_conv,
                (conversation_id.as_uuid(), bucket, subscriber_id.as_uuid()),
            )
            .await
            .map_err(scylla_err)?;

        let by_user = strict(
            &self.client,
            "DELETE FROM chat.subscriptions_by_user \
             WHERE subscriber_id = ? AND conversation_id = ?",
        );
        self.client
            .session
            .execute_unpaged(
                by_user,
                (subscriber_id.as_uuid(), conversation_id.as_uuid()),
            )
            .await
            .map_err(scylla_err)?;

        Ok(())
    }

    async fn is_subscribed(
        &self,
        subscriber_id:   &ProfileId,
        conversation_id: &ConversationId,
    ) -> Result<bool, ChatError> {
        let stmt = fast(
            &self.client,
            "SELECT conversation_id FROM chat.subscriptions_by_user \
             WHERE subscriber_id = ? AND conversation_id = ? LIMIT 1",
        );

        let found = self
            .client
            .session
            .execute_unpaged(stmt, (subscriber_id.as_uuid(), conversation_id.as_uuid()))
            .await
            .map_err(scylla_err)?
            .into_rows_result()
            .map_err(|e| row_err("subscription.is_subscribed:rows", e))?
            .rows::<(Uuid,)>()
            .map_err(|e| row_err("subscription.is_subscribed:iter", e))?
            .next()
            .is_some();

        Ok(found)
    }

    async fn list_by_user(
        &self,
        subscriber_id: &ProfileId,
        limit:         i32,
        cursor:        Option<Uuid>,
    ) -> Result<(Vec<ConversationId>, Option<Uuid>), ChatError> {
        let rows: Vec<(Uuid,)> = if let Some(after) = cursor {
            let stmt = analytical(
                &self.client,
                "SELECT conversation_id FROM chat.subscriptions_by_user \
                 WHERE subscriber_id = ? AND conversation_id > ? LIMIT ?",
            );
            self.client
                .session
                .execute_unpaged(stmt, (subscriber_id.as_uuid(), after, limit))
                .await
                .map_err(scylla_err)?
                .into_rows_result()
                .map_err(|e| row_err("subscription.list:rows", e))?
                .rows::<(Uuid,)>()
                .map_err(|e| row_err("subscription.list:iter", e))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| row_err("subscription.list:deser", e))?
        } else {
            let stmt = analytical(
                &self.client,
                "SELECT conversation_id FROM chat.subscriptions_by_user \
                 WHERE subscriber_id = ? LIMIT ?",
            );
            self.client
                .session
                .execute_unpaged(stmt, (subscriber_id.as_uuid(), limit))
                .await
                .map_err(scylla_err)?
                .into_rows_result()
                .map_err(|e| row_err("subscription.list:rows", e))?
                .rows::<(Uuid,)>()
                .map_err(|e| row_err("subscription.list:iter", e))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| row_err("subscription.list:deser", e))?
        };

        let next = if rows.len() == limit.max(0) as usize {
            rows.last().map(|(id,)| *id)
        } else {
            None
        };

        let ids = rows.into_iter().map(|(id,)| ConversationId::from_uuid(id)).collect();
        Ok((ids, next))
    }
}
