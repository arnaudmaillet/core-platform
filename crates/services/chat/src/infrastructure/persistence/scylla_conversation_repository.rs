use std::sync::Arc;

use async_trait::async_trait;
use scylla_storage::ScyllaClient;

use crate::application::port::ConversationRepository;
use crate::domain::aggregate::Conversation;
use crate::domain::value_object::{
    ConversationId, ConversationKind, MessageId, ProfileId, Visibility,
};
use crate::error::ChatError;
use crate::infrastructure::persistence::model::ConversationRow;
use crate::infrastructure::persistence::statement::{fast, row_err, scylla_err, strict};
use crate::infrastructure::persistence::time::{to_cql, to_utc};

/// ScyllaDB adapter for the [`Conversation`] aggregate (`chat.conversations`).
pub struct ScyllaConversationRepository {
    client: Arc<ScyllaClient>,
}

impl ScyllaConversationRepository {
    pub fn new(client: Arc<ScyllaClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl ConversationRepository for ScyllaConversationRepository {
    async fn insert(&self, c: &Conversation) -> Result<(), ChatError> {
        let stmt = strict(
            &self.client,
            "INSERT INTO chat.conversations \
             (conversation_id, kind, visibility, owner_id, member_count, public_since, \
              created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        );
        self.client
            .session
            .execute_unpaged(
                stmt,
                (
                    c.id().as_uuid(),
                    c.kind().as_tinyint(),
                    c.visibility().as_tinyint(),
                    c.owner_id().as_uuid(),
                    c.member_count() as i32,
                    c.public_since().map(|m| m.as_uuid()),
                    to_cql(c.created_at()),
                    to_cql(c.updated_at()),
                ),
            )
            .await
            .map_err(scylla_err)?;
        Ok(())
    }

    async fn update(&self, c: &Conversation) -> Result<(), ChatError> {
        // Only mutable columns are rewritten; kind/owner_id/created_at are immutable.
        let stmt = strict(
            &self.client,
            "UPDATE chat.conversations \
             SET visibility = ?, public_since = ?, member_count = ?, updated_at = ? \
             WHERE conversation_id = ?",
        );
        self.client
            .session
            .execute_unpaged(
                stmt,
                (
                    c.visibility().as_tinyint(),
                    c.public_since().map(|m| m.as_uuid()),
                    c.member_count() as i32,
                    to_cql(c.updated_at()),
                    c.id().as_uuid(),
                ),
            )
            .await
            .map_err(scylla_err)?;
        Ok(())
    }

    async fn find(&self, id: &ConversationId) -> Result<Option<Conversation>, ChatError> {
        let stmt = fast(
            &self.client,
            "SELECT conversation_id, kind, visibility, owner_id, member_count, public_since, \
                    created_at, updated_at \
             FROM chat.conversations WHERE conversation_id = ?",
        );

        let row = self
            .client
            .session
            .execute_unpaged(stmt, (id.as_uuid(),))
            .await
            .map_err(scylla_err)?
            .into_rows_result()
            .map_err(|e| row_err("conversation.find:rows", e))?
            .rows::<ConversationRow>()
            .map_err(|e| row_err("conversation.find:iter", e))?
            .next();

        let Some(row) = row else { return Ok(None) };
        let row = row.map_err(|e| row_err("conversation.find:deser", e))?;

        Ok(Some(Conversation::reconstitute(
            ConversationId::from_uuid(row.conversation_id),
            ConversationKind::try_from(row.kind)?,
            Visibility::try_from(row.visibility)?,
            ProfileId::from_uuid(row.owner_id),
            row.member_count.max(0) as u16,
            row.public_since.map(MessageId::from_uuid),
            to_utc(row.created_at),
            to_utc(row.updated_at),
        )))
    }
}
