// crates/profile/src/application/use_cases/get_nearby_users/dto.rs

use serde::Serialize;
use shared_kernel::domain::entities::GeoPoint;
use shared_kernel::domain::value_objects::AccountId;

#[derive(Serialize)]
pub struct NearbyUserDto {
    pub account_id: AccountId,
    pub coordinates: GeoPoint,
    pub distance_meters: f64,
    pub is_obfuscated: bool,
}
