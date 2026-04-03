use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_proto::account::v1::DeactivateAccountRequest;

#[derive(Debug, Deserialize, Clone)]
pub struct DeactivateCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
}

impl DeactivateCommand {
    pub fn try_from_proto(proto: DeactivateAccountRequest, region: RegionCode) -> Result<Self, tonic::Status> {
        Ok(Self {
            account_id: AccountId::try_from(proto.id) // Accès au champ .id du proto
                .map_err(|e| tonic::Status::invalid_argument(format!("Invalid AccountId: {}", e)))?,
            region_code: region,
        })
    }
}