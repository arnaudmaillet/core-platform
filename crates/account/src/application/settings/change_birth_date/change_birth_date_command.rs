// crates/account/src/application/change_birth_date/command.rs

use crate::domain::value_objects::BirthDate;
use serde::Deserialize;
use shared_kernel::{domain::value_objects::{AccountId, RegionCode}, infrastructure::grpc::ProtoTimestampExt};
use shared_proto::account::v1::ChangeBirthDateRequest;
use tonic::Status;

#[derive(Debug, Deserialize, Clone)]
pub struct ChangeBirthDateCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub birth_date: BirthDate,
}

impl ChangeBirthDateCommand {
    pub fn try_from_proto(req: ChangeBirthDateRequest, region: RegionCode) -> Result<Self, tonic::Status> {
        let birth_date = req.new_birth_date
            .and_then(|ts| ts.to_naive_date())
            .and_then(|date| BirthDate::try_new(date).ok())
            .ok_or_else(|| Status::invalid_argument("Invalid or missing birth date"))?;
        Ok(Self {
            account_id: AccountId::try_from(req.id)
                .map_err(|e| tonic::Status::invalid_argument(format!("Invalid AccountId: {}", e)))?,
            region_code: region,
            birth_date: birth_date,
        })
    }
}
