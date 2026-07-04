use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use uuid::Uuid;
use validate_core::{FieldViolation, Validate};

use crate::application::port::{NotificationRepository, UnreadCounter};
use crate::domain::value_object::ProfileId;
use crate::error::NotificationError;

// ── MarkReadCommand ───────────────────────────────────────────────────────────

/// Marks a single notification as read and decrements the unread counter.
///
/// The client must supply `created_at_ms` because ScyllaDB requires the full
/// compound clustering key `(created_at, notification_id)` for a point UPDATE.
pub struct MarkReadCommand {
    pub profile_id:      String,
    pub notification_id: String,
    pub created_at_ms:   i64,
}

impl Command for MarkReadCommand {}

impl Validate for MarkReadCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.profile_id.trim().is_empty() {
            v.push(FieldViolation::new("profile_id", "NTF-VAL-010", "profile_id must not be empty"));
        }
        if self.notification_id.trim().is_empty() {
            v.push(FieldViolation::new("notification_id", "NTF-VAL-011", "notification_id must not be empty"));
        }
        if self.created_at_ms <= 0 {
            v.push(FieldViolation::new("created_at_ms", "NTF-VAL-012", "created_at_ms must be a positive Unix timestamp in milliseconds"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct MarkReadHandler<R, U> {
    pub repository: Arc<R>,
    pub counter:    Arc<U>,
}

impl<R, U> CommandHandler<MarkReadCommand> for MarkReadHandler<R, U>
where
    R: NotificationRepository,
    U: UnreadCounter,
{
    type Error = NotificationError;

    async fn handle(
        &self,
        envelope: Envelope<MarkReadCommand>,
    ) -> Result<(), NotificationError> {
        let cmd = &envelope.payload;

        let profile_id = ProfileId::try_from(cmd.profile_id.as_str())?;
        let ntf_id     = Uuid::parse_str(&cmd.notification_id)
            .map_err(|_| NotificationError::InvalidNotificationId(cmd.notification_id.clone()))?;

        let was_unread = self.repository
            .mark_read(&profile_id, ntf_id, cmd.created_at_ms)
            .await?;

        if was_unread {
            self.counter.decrement(&profile_id).await?;

            tracing::debug!(
                notification_id = %ntf_id,
                profile_id      = %profile_id,
                "notification marked as read, counter decremented"
            );
        }

        Ok(())
    }
}

// ── MarkAllReadCommand ────────────────────────────────────────────────────────

/// Resets the unread counter to 0 and records a read_horizon timestamp.
///
/// Does NOT update individual `is_read` flags in ScyllaDB — this would require
/// a full partition scan with no bound on cost. The client applies the
/// `read_horizon_ms` field from `ListNotificationsResponse` locally.
pub struct MarkAllReadCommand {
    pub profile_id: String,
}

impl Command for MarkAllReadCommand {}

impl Validate for MarkAllReadCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        if self.profile_id.trim().is_empty() {
            return Err(vec![FieldViolation::new(
                "profile_id",
                "NTF-VAL-020",
                "profile_id must not be empty",
            )]);
        }
        Ok(())
    }
}

pub struct MarkAllReadHandler<R, U> {
    pub repository: Arc<R>,
    pub counter:    Arc<U>,
}

impl<R, U> CommandHandler<MarkAllReadCommand> for MarkAllReadHandler<R, U>
where
    R: NotificationRepository,
    U: UnreadCounter,
{
    type Error = NotificationError;

    async fn handle(
        &self,
        envelope: Envelope<MarkAllReadCommand>,
    ) -> Result<(), NotificationError> {
        let profile_id = ProfileId::try_from(envelope.payload.profile_id.as_str())?;
        let horizon_ms = chrono::Utc::now().timestamp_millis();

        tokio::try_join!(
            self.repository.reset_counter(&profile_id),
            self.counter.reset(&profile_id),
        )?;

        self.counter.set_read_horizon(&profile_id, horizon_ms).await?;

        tracing::debug!(
            profile_id  = %profile_id,
            horizon_ms,
            "all notifications marked as read"
        );

        Ok(())
    }
}
