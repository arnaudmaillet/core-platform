// crates/post/src/infrastructure/profile/scylla/models.rs

use infra_scylla::scylla;
use uuid::Uuid;

/// POUR LA LECTURE : Hydratée automatiquement par Scylla
#[derive(scylla::DeserializeRow, Debug, Clone)]
pub struct ScyllaProfileModel {
    pub profile_id: Uuid,
    pub handle: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub is_verified: bool,
}

/// POUR L'ÉCRITURE : Structure optimisée avec Lifetimes (zéro-allocation)
pub struct ScyllaProfileUpdateModel<'a> {
    pub profile_id: Uuid,
    pub handle: &'a str,
    pub display_name: &'a str,
    pub avatar_url: Option<&'a str>,
    pub is_verified: bool,
}
