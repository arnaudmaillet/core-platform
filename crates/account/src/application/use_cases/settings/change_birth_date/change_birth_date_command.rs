// crates/account/src/application/change_birth_date/command.rs

use crate::domain::value_objects::BirthDate;
use serde::Deserialize;
use shared_kernel::{
    domain::value_objects::AccountId,
    errors::{DomainError, Result},
    infrastructure::grpc::ProtoTimestampExt,
};
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
    pub fn try_from_proto(req: ChangeBirthDateRequest) -> Result<Self> {
        let new_birth_date = req
            .new_birth_date
            .and_then(|ts| ts.to_naive_date())
            .ok_or_else(|| DomainError::Validation {
                field: "new_birth_date",
                reason: "Missing or invalid timestamp".to_string(),
            })
            .and_then(|date| {
                BirthDate::try_new(date).map_err(|e| DomainError::Validation {
                    field: "new_birth_date",
                    reason: e.to_string(),
                })
            })?;

        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id).map_err(|_| DomainError::Validation {
                field: "command_id",
                reason: "Invalid UUID format".to_string(),
            })?,

            account_id: AccountId::try_new(&req.account_id).map_err(|e| {
                DomainError::Validation {
                    field: "account_id",
                    reason: e.to_string(),
                }
            })?,

            new_birth_date,
        })
    }
}
