use serde::Deserialize;
use shared_kernel::domain::value_objects::AccountId;
use shared_proto::account::v1::VerifyEmailRequest;
use uuid::Uuid;

use crate::domain::value_objects::VerificationToken;

#[derive(Debug, Deserialize, Clone)]
pub struct VerifyEmailCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub token: VerificationToken,
}

impl VerifyEmailCommand {
    pub fn try_from_proto(req: VerifyEmailRequest) -> Result<Self, tonic::Status> {
        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid CommandId: {}", e))
            })?,
            account_id: AccountId::try_from(req.account_id).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid AccountId: {}", e))
            })?,
            token: VerificationToken::try_new(req.token)
                .map_err(|e| tonic::Status::invalid_argument(format!("Invalid Token: {}", e)))?,
        })
    }
}
