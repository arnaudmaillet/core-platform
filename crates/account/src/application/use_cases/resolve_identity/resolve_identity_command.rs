// crates/account/src/application/resolve_identity/resolve_identity_command.rs

use shared_proto::account::v1::ResolveIdentityRequest;

use crate::domain::value_objects::ExternalId;

#[derive(Debug, Clone)]
pub struct ResolveIdentityCommand {
    pub external_id: ExternalId,
}

impl ResolveIdentityCommand {
    pub fn try_from_proto(req: ResolveIdentityRequest) -> Result<Self, tonic::Status> {
        Ok(Self {
            external_id: ExternalId::try_from(req.external_id)
                .map_err(|e| tonic::Status::invalid_argument(format!("Invalid ExternalId: {}", e)))?,
        })
    }
}