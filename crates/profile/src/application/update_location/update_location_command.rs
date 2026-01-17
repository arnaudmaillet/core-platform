
use shared_kernel::domain::value_objects::{GeoPoint, RegionCode, AccountId};
use crate::domain::value_objects::{LocationMetrics, MovementMetrics};

pub struct UpdateLocationCommand {
    pub account_id: AccountId,
    pub region: RegionCode,
    pub coords: GeoPoint,
    pub metrics: Option<LocationMetrics>,
    pub movement: Option<MovementMetrics>,
}