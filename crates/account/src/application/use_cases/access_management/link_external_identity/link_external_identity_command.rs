// crates/account/src/application/link_external_identity/link_external_identity_command.rs

use crate::domain::value_objects::ExternalId;
use shared_kernel::domain::value_objects::AccountId;
use shared_proto::account::v1::LinkExternalIdentityRequest;

#[derive(Debug, Clone)]
pub struct LinkExternalIdentityCommand {
    pub account_id: AccountId,
    pub external_id: ExternalId,
}

impl LinkExternalIdentityCommand {
    pub fn try_from_proto(req: LinkExternalIdentityRequest) -> Result<Self, tonic::Status> {
        Ok(Self {
            account_id: AccountId::try_from(req.account_id).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid AccountId: {}", e))
            })?,
            external_id: ExternalId::try_from(req.external_id).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid ExternalId: {}", e))
            })?,
        })
    }
}
