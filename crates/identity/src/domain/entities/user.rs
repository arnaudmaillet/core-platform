// crates/identity/src/entities/user

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::value_objects::{
    UserId, Username, Email, PhoneNumber, DisplayName, Bio, BirthDate,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub cognito_sub: String,

    pub username: Username,
    pub email: Option<Email>,
    pub phone_number: Option<PhoneNumber>,

    pub email_verified: bool,
    pub phone_verified: bool,

    pub display_name: Option<DisplayName>,
    pub bio: Option<Bio>,
    pub birth_date: Option<BirthDate>,

    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    #[allow(clippy::too_many_arguments)]
    pub fn from_cognito(
        id: UserId,
        cognito_sub: String,
        username: Username,
        email: Option<Email>,
        phone_number: Option<PhoneNumber>,
        email_verified: bool,
        phone_verified: bool,
        display_name: Option<DisplayName>,
        birth_date: Option<BirthDate>,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            cognito_sub,
            username,
            email,
            phone_number,
            email_verified,
            phone_verified,
            display_name,
            bio: None,
            birth_date,
            avatar_url: None,
            created_at,
            updated_at: created_at,
        }
    }

    pub fn update_profile(
        &mut self,
        display_name: Option<DisplayName>,
        bio: Option<Bio>,
        birth_date: Option<BirthDate>,
        avatar_url: Option<String>,
        updated_at: DateTime<Utc>,
    ) {
        if let Some(dn) = display_name {
            self.display_name = Some(dn);
        }
        if let Some(b) = bio {
            self.bio = Some(b);
        }
        if let Some(bd) = birth_date {
            self.birth_date = Some(bd);
        }
        if let Some(av) = avatar_url {
            self.avatar_url = Some(av);
        }
        self.updated_at = updated_at;
    }
}