use sqlx::types::Json;
use uuid::Uuid;

use crate::domain::aggregate::PenaltyLedger;
use crate::domain::value_object::{ActorId, Strike};

/// Flat projection of the `penalty_ledgers` table.
#[derive(Debug, sqlx::FromRow)]
pub struct PenaltyRow {
    pub actor_id: Uuid,
    pub strikes: Json<Vec<Strike>>,
    pub version: i64,
}

impl From<PenaltyRow> for PenaltyLedger {
    fn from(row: PenaltyRow) -> Self {
        PenaltyLedger::reconstitute(ActorId::from_uuid(row.actor_id), row.strikes.0, row.version)
    }
}
