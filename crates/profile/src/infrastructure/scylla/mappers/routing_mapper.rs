use infra_scylla::scylla;
use uuid::Uuid;

#[derive(scylla::DeserializeRow)]
pub struct ScyllaRouterProfileRow {
    pub region: String,
}

#[derive(scylla::DeserializeRow)]
pub struct ScyllaRouterSlugRow {
    pub profile_id: Uuid,
    pub region: String,
}
