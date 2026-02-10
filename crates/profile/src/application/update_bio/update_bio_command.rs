// crates/profile/src/application/use_cases/update_bio/update_bio_command.rs

use crate::domain::value_objects::{Bio, ProfileId};
use serde::{Deserialize, Serialize};
use tonic::Status;
use shared_kernel::domain::value_objects::RegionCode;
use crate::infrastructure::api::grpc::profile_v1::UpdateBioRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateBioCommand {
    pub profile_id: ProfileId,
    pub region: RegionCode,
    pub new_bio: Option<Bio>,
}


impl UpdateBioCommand {
    pub fn try_from_proto(req: UpdateBioRequest, region: RegionCode) -> Result<Self, Status> {
        let profile_id = ProfileId::try_from(req.profile_id)
            .map_err(|e| Status::invalid_argument(format!("ProfileId: {}", e)))?;

        // Nettoyage : si c'est vide ou juste des espaces, on traite comme None
        let new_bio = req.new_bio
            .filter(|s| !s.trim().is_empty())
            .map(|s| Bio::try_from(s).map_err(|e| Status::invalid_argument(e.to_string())))
            .transpose()?;

        Ok(Self { profile_id, region, new_bio })
    }
}