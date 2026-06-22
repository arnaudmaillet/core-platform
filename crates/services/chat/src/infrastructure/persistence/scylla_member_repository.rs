use std::sync::Arc;

use async_trait::async_trait;
use scylla_storage::ScyllaClient;

use crate::application::port::MemberRepository;
use crate::domain::aggregate::Participant;
use crate::domain::value_object::{ConversationId, MessageId, ProfileId, Role};
use crate::error::ChatError;
use crate::infrastructure::persistence::model::MemberRow;
use crate::infrastructure::persistence::statement::{fast, row_err, scylla_err, strict};
use crate::infrastructure::persistence::time::{to_cql, to_utc};

const MEMBER_COLS: &str = "member_id, role, joined_at, last_read";

/// ScyllaDB adapter for the bounded Member Plane roster
/// (`chat.members_by_conversation`).
pub struct ScyllaMemberRepository {
    client: Arc<ScyllaClient>,
}

impl ScyllaMemberRepository {
    pub fn new(client: Arc<ScyllaClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl MemberRepository for ScyllaMemberRepository {
    async fn insert(
        &self,
        conversation_id: &ConversationId,
        p:               &Participant,
    ) -> Result<(), ChatError> {
        let stmt = strict(
            &self.client,
            "INSERT INTO chat.members_by_conversation \
             (conversation_id, member_id, role, joined_at, last_read) \
             VALUES (?, ?, ?, ?, ?)",
        );
        self.client
            .session
            .execute_unpaged(
                stmt,
                (
                    conversation_id.as_uuid(),
                    p.profile_id().as_uuid(),
                    p.role().as_tinyint(),
                    to_cql(p.joined_at()),
                    p.last_read().map(|m| m.as_uuid()),
                ),
            )
            .await
            .map_err(scylla_err)?;
        Ok(())
    }

    async fn find(
        &self,
        conversation_id: &ConversationId,
        member_id:       &ProfileId,
    ) -> Result<Option<Participant>, ChatError> {
        let stmt = fast(
            &self.client,
            &format!(
                "SELECT {MEMBER_COLS} FROM chat.members_by_conversation \
                 WHERE conversation_id = ? AND member_id = ?"
            ),
        );

        let row = self
            .client
            .session
            .execute_unpaged(stmt, (conversation_id.as_uuid(), member_id.as_uuid()))
            .await
            .map_err(scylla_err)?
            .into_rows_result()
            .map_err(|e| row_err("member.find:rows", e))?
            .rows::<MemberRow>()
            .map_err(|e| row_err("member.find:iter", e))?
            .next();

        let Some(row) = row else { return Ok(None) };
        let row = row.map_err(|e| row_err("member.find:deser", e))?;

        Ok(Some(participant_from_row(row)?))
    }

    async fn update_last_read(
        &self,
        conversation_id: &ConversationId,
        member_id:       &ProfileId,
        last_read:       MessageId,
    ) -> Result<(), ChatError> {
        let stmt = strict(
            &self.client,
            "UPDATE chat.members_by_conversation SET last_read = ? \
             WHERE conversation_id = ? AND member_id = ?",
        );
        self.client
            .session
            .execute_unpaged(
                stmt,
                (last_read.as_uuid(), conversation_id.as_uuid(), member_id.as_uuid()),
            )
            .await
            .map_err(scylla_err)?;
        Ok(())
    }

    async fn list(
        &self,
        conversation_id: &ConversationId,
    ) -> Result<Vec<Participant>, ChatError> {
        // Single bounded partition (<= 500 rows) — a full clustering scan is safe.
        let stmt = fast(
            &self.client,
            &format!(
                "SELECT {MEMBER_COLS} FROM chat.members_by_conversation \
                 WHERE conversation_id = ?"
            ),
        );

        let rows = self
            .client
            .session
            .execute_unpaged(stmt, (conversation_id.as_uuid(),))
            .await
            .map_err(scylla_err)?
            .into_rows_result()
            .map_err(|e| row_err("member.list:rows", e))?
            .rows::<MemberRow>()
            .map_err(|e| row_err("member.list:iter", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| row_err("member.list:deser", e))?;

        rows.into_iter().map(participant_from_row).collect()
    }

    async fn delete(
        &self,
        conversation_id: &ConversationId,
        member_id:       &ProfileId,
    ) -> Result<(), ChatError> {
        let stmt = strict(
            &self.client,
            "DELETE FROM chat.members_by_conversation \
             WHERE conversation_id = ? AND member_id = ?",
        );
        self.client
            .session
            .execute_unpaged(stmt, (conversation_id.as_uuid(), member_id.as_uuid()))
            .await
            .map_err(scylla_err)?;
        Ok(())
    }
}

fn participant_from_row(row: MemberRow) -> Result<Participant, ChatError> {
    Ok(Participant::reconstitute(
        ProfileId::from_uuid(row.member_id),
        Role::try_from(row.role)?,
        to_utc(row.joined_at),
        row.last_read.map(MessageId::from_uuid),
    ))
}
