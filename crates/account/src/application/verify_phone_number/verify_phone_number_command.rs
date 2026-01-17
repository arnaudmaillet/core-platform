// crates/account/src/application/verify_phone_number/command.rs

use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, Deserialize, Clone)]
pub struct VerifyPhoneNumberCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub code: String, // Généralement un code OTP à 6 chiffres reçu par SMS
}