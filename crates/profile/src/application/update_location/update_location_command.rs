use crate::domain::value_objects::{LocationMetrics, MovementMetrics};
use shared_kernel::domain::entities::GeoPoint;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

pub struct UpdateLocationCommand {
    pub account_id: AccountId,
    pub region: RegionCode,
    pub coords: GeoPoint,
    pub metrics: Option<LocationMetrics>,
    pub movement: Option<MovementMetrics>,
}
