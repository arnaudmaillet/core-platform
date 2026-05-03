use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::{Email, PhoneNumber, SubId};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    #[serde(rename = "sub")]
    pub sub_id: SubId,
    pub email: Option<Email>,
    pub email_verified: Option<bool>,
    pub phone_number: Option<PhoneNumber>,
    pub realm_access: Option<RealmAccess>,
    pub exp: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RealmAccess {
    pub roles: Vec<String>,
}
