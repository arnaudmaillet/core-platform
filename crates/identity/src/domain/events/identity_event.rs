// crates/identity/src/events/identity_event.rs

use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

use crate::entities::User;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IdentityEvent {
    UserCreated {
        user: User,
        occurred_at: DateTime<Utc>,
    },

    ProfileUpdated {
        user_id: String,
        changes: ProfileChanges,
        occurred_at: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileChanges {
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub birth_date: Option<String>, // ou ton format
}