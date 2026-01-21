// crates/profile/src/infrastructure/api/grpc/mappers/location.rs

use shared_kernel::domain::value_objects::GeoPoint;
use crate::domain::entities::UserLocation;
use crate::domain::value_objects::{LocationMetrics, MovementMetrics};
use crate::infrastructure::api::grpc::mappers::grpc_common_mapper::to_timestamp;
use super::super::location_v1::{
    UserLocation as ProtoUserLocation,
    GeoPoint as ProtoGeoPoint,
    LocationMetrics as ProtoLocationMetrics,
    MovementMetrics as ProtoMovementMetrics,
};

// --- Domaine -> Proto ---

impl From<UserLocation> for ProtoUserLocation {
    fn from(domain: UserLocation) -> Self {
        Self {
            account_id: domain.account_id.to_string(),
            region_code: domain.region_code.to_string(),
            coordinates: Some(domain.coordinates.into()),

            metrics: domain.metrics.map(Into::into),
            movement: domain.movement.map(Into::into),

            is_ghost_mode: domain.is_ghost_mode,
            privacy_radius_meters: domain.privacy_radius_meters,

            updated_at: Some(to_timestamp(domain.updated_at)),
            version: domain.metadata.version as i64,
        }
    }
}

impl From<GeoPoint> for ProtoGeoPoint {
    fn from(domain: GeoPoint) -> Self {
        Self {
            latitude: domain.lat(),
            longitude: domain.lon(),
        }
    }
}

impl From<LocationMetrics> for ProtoLocationMetrics {
    fn from(domain: LocationMetrics) -> Self {
        Self {
            accuracy: domain.accuracy().value(),
            altitude: domain.altitude().map(|a| a.value().into()),
        }
    }
}

impl From<MovementMetrics> for ProtoMovementMetrics {
    fn from(domain: MovementMetrics) -> Self {
        Self {
            speed: domain.speed().value(),
            heading: domain.heading().value(),
        }
    }
}