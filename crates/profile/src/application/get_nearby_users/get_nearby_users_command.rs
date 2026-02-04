// crates/profile/src/application/use_cases/get_nearby_users/query.rs

use shared_kernel::domain::entities::GeoPoint;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

pub struct GetNearbyUsersCommand {
    pub account_id: AccountId, // L'utilisateur qui regarde
    pub center: GeoPoint,      // Sa position actuelle
    pub region: RegionCode,    // Sa région de sharding
    pub radius_meters: f64,    // Rayon de recherche (ex: 2000.0)
    pub limit: i64,            // Pagination (ex: 50)
}
