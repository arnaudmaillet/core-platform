// crates/profile/src/application/use_cases/get_nearby_users/dto.rs

use serde::Serialize;
use shared_kernel::domain::entities::GeoPoint;
use crate::domain::value_objects::ProfileId;

#[derive(Serialize)]
pub struct NearbyUserDto {
    pub profile_id: ProfileId,
    pub coordinates: GeoPoint,
    pub distance_meters: f64,
    pub is_obfuscated: bool,
}
