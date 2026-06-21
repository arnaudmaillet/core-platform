use infra_scylla::scylla;
use uuid::Uuid;

#[derive(scylla::DeserializeRow)]
pub struct CqlRouterProfileRow {
    pub region: String,
}

#[derive(scylla::DeserializeRow)]
pub struct CqlRouterSlugRow {
    pub profile_id: Uuid,
    pub region: String,
}
