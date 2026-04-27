// crates/account/src/application/change_email/change_email_command.rs

use crate::domain::value_objects::Email;
use shared_kernel::domain::value_objects::AccountId;
use shared_proto::account::v1::ChangeEmailRequest;
use uuid::Uuid;

#[derive(Clone)]
pub struct ChangeEmailCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub new_email: Email,
}

impl ChangeEmailCommand {
    pub fn try_from_proto(req: ChangeEmailRequest) -> Result<Self, tonic::Status> {
        Ok(Self {            
            command_id: Uuid::parse_str(&req.command_id).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid CommandId: {}", e))
            })?,
            account_id: AccountId::try_from(req.account_id).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid AccountId: {}", e))
            })?,
            new_email: Email::try_from(req.new_email)
                .map_err(|e| tonic::Status::invalid_argument(format!("Invalid Email: {}", e)))?,
        })
    }
}
