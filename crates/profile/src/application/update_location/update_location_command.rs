use crate::domain::value_objects::{LocationMetrics, MovementMetrics, ProfileId};
use shared_kernel::domain::entities::GeoPoint;
use shared_kernel::domain::value_objects::RegionCode;

pub struct UpdateLocationCommand {
    pub profile_id: ProfileId,
    pub region: RegionCode,
    pub coords: GeoPoint,
    pub metrics: Option<LocationMetrics>,
    pub movement: Option<MovementMetrics>,
}
