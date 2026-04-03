// crates/account/src/application/verify_phone_number/command.rs

use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_proto::account::v1::VerifyPhoneNumberRequest;

#[derive(Debug, Deserialize, Clone)]
pub struct VerifyPhoneNumberCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub code: String,
}

impl VerifyPhoneNumberCommand {
    pub fn try_from_proto(req: VerifyPhoneNumberRequest, region: RegionCode) -> Result<Self, tonic::Status> {
        Ok(Self {
            account_id: AccountId::try_from(req.id)
                .map_err(|e| tonic::Status::invalid_argument(format!("Invalid AccountId: {}", e)))?,
            region_code: region,
            code: req.code,
        })
    }
}