// crates/post/src/infrastructure/profile/scylla/mapper.rs

use crate::infrastructure::profile::scylla::{ScyllaProfileModel, ScyllaProfileUpdateModel};
use shared_kernel::core::Error;
use shared_proto::profile::v1::ProfileSummaryDto;
use uuid::Uuid;

/// Convertit la ligne Scylla en DTO Protobuf (Infaillible)
impl From<ScyllaProfileModel> for ProfileSummaryDto {
    fn from(row: ScyllaProfileModel) -> Self {
        Self {
            profile_id: row.profile_id.to_string(),
            handle: row.handle,
            display_name: row.display_name,
            avatar_url: row.avatar_url,
            is_verified: row.is_verified,
        }
    }
}

/// Permet de préparer l'update à partir du DTO (Peut échouer à cause du parsing UUID)
impl<'a> ScyllaProfileUpdateModel<'a> {
    pub fn try_from_dto(dto: &'a ProfileSummaryDto) -> Result<Self, Error> {
        let profile_uuid = Uuid::parse_str(&dto.profile_id).map_err(|e| {
            Error::validation(
                "profile_id",
                format!("Invalid projection UUID format: {}", e),
            )
        })?;

        Ok(Self {
            profile_id: profile_uuid,
            handle: &dto.handle,
            display_name: &dto.display_name,
            avatar_url: dto.avatar_url.as_deref(),
            is_verified: dto.is_verified,
        })
    }
}
