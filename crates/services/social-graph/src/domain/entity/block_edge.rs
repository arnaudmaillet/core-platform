use chrono::{DateTime, Utc};

use crate::domain::value_object::ProfileId;

/// A single directed block edge as returned by block-list queries.
#[derive(Debug, Clone)]
pub struct BlockEdge {
    pub blockee_id: ProfileId,
    pub blocked_at: DateTime<Utc>,
}
