// crates/post/profile/src/infrastructure/scylla/mapper.rs

use crate::ProjectedProfile;
use crate::infrastructure::scylla::{ScyllaProfileModel, ScyllaProfileUpdateModel};
use shared_kernel::core::Identifier;
use shared_kernel::types::ProfileId;

/// Convertit la ligne Scylla brute vers l'entité pur du Domaine
impl From<ScyllaProfileModel> for ProjectedProfile {
    fn from(row: ScyllaProfileModel) -> Self {
        Self {
            id: ProfileId::from(row.profile_id),
            handle: row.handle,
            display_name: row.display_name,
            avatar_url: row.avatar_url,
            is_verified: row.is_verified,
        }
    }
}
/// Prépare l'écriture Scylla à partir du Domaine
impl<'a> From<&'a ProjectedProfile> for ScyllaProfileUpdateModel<'a> {
    fn from(domain: &'a ProjectedProfile) -> Self {
        Self {
            profile_id: domain.id.as_uuid(),
            handle: &domain.handle,
            display_name: &domain.display_name,
            avatar_url: domain.avatar_url.as_deref(),
            is_verified: domain.is_verified,
        }
    }
}
