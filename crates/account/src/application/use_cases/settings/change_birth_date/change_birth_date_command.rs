// crates/account/src/application/change_birth_date/command.rs

use crate::domain::value_objects::BirthDate;
use serde::Deserialize;
use shared_kernel::{domain::value_objects::AccountId, infrastructure::grpc::ProtoTimestampExt};
use shared_proto::account::v1::ChangeBirthDateRequest;
use tonic::Status;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct ChangeBirthDateCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub new_birth_date: BirthDate,
}

impl ChangeBirthDateCommand {
    pub fn try_from_proto(req: ChangeBirthDateRequest) -> Result<Self, tonic::Status> {
        let new_birth_date = req
            .new_birth_date
            .and_then(|ts| ts.to_naive_date())
            .and_then(|date| BirthDate::try_new(date).ok())
            .ok_or_else(|| Status::invalid_argument("Invalid or missing birth date"))?;
        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid CommandId: {}", e))
            })?,
            account_id: AccountId::try_from(req.account_id).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid AccountId: {}", e))
            })?,
            new_birth_date: new_birth_date,
        })
    }
}
