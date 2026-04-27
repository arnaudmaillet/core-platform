// crates/account/src/application/verify_phone_number/command.rs

use serde::Deserialize;
use shared_kernel::domain::value_objects::AccountId;
use shared_proto::account::v1::VerifyPhoneNumberRequest;

use crate::domain::value_objects::VerificationCode;

#[derive(Debug, Deserialize, Clone)]
pub struct VerifyPhoneNumberCommand {
    pub account_id: AccountId,
    pub code: VerificationCode,
}

impl VerifyPhoneNumberCommand {
    pub fn try_from_proto(req: VerifyPhoneNumberRequest) -> Result<Self, tonic::Status> {
        Ok(Self {
            account_id: AccountId::try_from(req.account_id).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid AccountId: {}", e))
            })?,
            code: VerificationCode::try_from(req.code).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid VerificationCode: {}", e))
            })?,
        })
    }
}
