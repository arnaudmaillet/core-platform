// crates/account/src/domain/params/patch_user_params.rs

use crate::domain::value_objects::{AccountState, BirthDate, Email, Locale, PhoneNumber};
use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::Username;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PatchUserParams {
    pub username: Option<Username>,
    pub email: Option<Email>,
    pub email_verified: Option<bool>,
    pub phone_number: Option<PhoneNumber>,
    pub phone_verified: Option<bool>,
    pub state: Option<AccountState>,
    pub birth_date: Option<BirthDate>,
    pub locale: Option<Locale>,
}

impl PatchUserParams {
    pub fn is_empty(&self) -> bool {
        self.username.is_none()
            && self.email.is_none()
            && self.email_verified.is_none()
            && self.phone_number.is_none()
            && self.phone_verified.is_none()
            && self.state.is_none()
            && self.birth_date.is_none()
            && self.locale.is_none()
    }
}
