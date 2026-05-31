// crates/auth/src/domain/claims.rs

use serde::{Deserialize, Serialize};
use shared_kernel::types::{Email, PhoneNumber, SubId};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    #[serde(rename = "sub")]
    pub sub_id: SubId,
    pub email: Option<Email>,
    pub email_verified: Option<bool>,
    pub phone_number: Option<PhoneNumber>,
    pub phone_number_verified: Option<bool>,
    pub realm_access: Option<RealmAccess>,
    pub exp: u64,
    pub aud: serde_json::Value,
    pub iss: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RealmAccess {
    pub roles: Vec<String>,
}
