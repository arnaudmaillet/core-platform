use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};
use uuid::Uuid;

use crate::application::port::{NotificationRepository, NotificationSummary, UnreadCounter};
use crate::domain::value_object::ProfileId;
use crate::error::NotificationError;

pub struct NotificationPage {
    pub notifications:   Vec<NotificationSummary>,
    pub next_page_token: Option<String>,
    pub read_horizon_ms: i64,
}

pub struct ListNotificationsQuery {
    pub profile_id: String,
    pub limit:      i32,
    /// Opaque cursor from the previous `next_page_token`.
    /// Encoding: `"{created_at_ms}_{notification_id}"`.
    pub page_token: Option<String>,
}

impl Query for ListNotificationsQuery {
    type Response = NotificationPage;
}

pub struct ListNotificationsHandler<R, U> {
    pub repository: Arc<R>,
    pub counter:    Arc<U>,
    pub max_page_size: i32,
}

impl<R, U> QueryHandler<ListNotificationsQuery> for ListNotificationsHandler<R, U>
where
    R: NotificationRepository,
    U: UnreadCounter,
{
    type Error = NotificationError;

    async fn handle(
        &self,
        envelope: Envelope<ListNotificationsQuery>,
    ) -> Result<NotificationPage, NotificationError> {
        let query = &envelope.payload;

        let profile_id = ProfileId::try_from(query.profile_id.as_str())?;
        let limit      = query.limit.min(self.max_page_size).max(1);
        let cursor     = query.page_token.as_deref().map(decode_cursor).transpose()?;

        let (notifications, next_page_token) = self.repository
            .list_paginated(&profile_id, limit, cursor)
            .await?;

        let read_horizon_ms = self.counter.get_read_horizon(&profile_id).await?;

        Ok(NotificationPage {
            notifications,
            next_page_token,
            read_horizon_ms,
        })
    }
}

/// Decodes a page cursor from its string representation.
/// Format: `"{created_at_ms}_{notification_id}"`.
fn decode_cursor(token: &str) -> Result<(i64, Uuid), NotificationError> {
    let (ts_str, id_str) = token.split_once('_').ok_or_else(|| {
        NotificationError::InvalidPageToken { token: token.to_owned() }
    })?;

    let ts = ts_str.parse::<i64>().map_err(|_| NotificationError::InvalidPageToken {
        token: token.to_owned(),
    })?;

    let id = Uuid::parse_str(id_str).map_err(|_| NotificationError::InvalidPageToken {
        token: token.to_owned(),
    })?;

    Ok((ts, id))
}

/// Encodes a cursor from the last row of a page.
pub fn encode_cursor(created_at_ms: i64, notification_id: Uuid) -> String {
    format!("{}_{}", created_at_ms, notification_id)
}
