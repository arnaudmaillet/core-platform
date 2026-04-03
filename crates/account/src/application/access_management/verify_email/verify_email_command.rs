use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_proto::account::v1::VerifyEmailRequest;

pub struct VerifyEmailCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub token: String,
}

impl VerifyEmailCommand {
    pub fn try_from_proto(req: VerifyEmailRequest, region: RegionCode) -> Result<Self, tonic::Status> {
        Ok(Self {
            account_id: AccountId::try_from(req.id)
                .map_err(|e| tonic::Status::invalid_argument(format!("Invalid AccountId: {}", e)))?,
            region_code: region,
            token: req.token,
        })
    }
}