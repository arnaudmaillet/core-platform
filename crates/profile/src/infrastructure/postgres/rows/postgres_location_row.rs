// crates/profile/src/infrastructure/postgres/rows/postgres_location_row.rs

use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use shared_kernel::domain::entities::GeoPoint;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::value_objects::{Altitude, Heading, LocationAccuracy, RegionCode, Speed, AccountId};
use shared_kernel::errors::{Result, DomainError};
use crate::domain::entities::UserLocation;
use crate::domain::builders::UserLocationBuilder;
use crate::domain::value_objects::{LocationMetrics, MovementMetrics};

#[derive(FromRow)]
pub struct PostgresLocationRow {
    pub account_id: Uuid,
    pub region_code: String,
    pub lon: f64,
    pub lat: f64,
    pub accuracy_meters: Option<f32>,
    pub altitude: Option<f32>,
    pub heading: Option<f32>,
    pub speed: Option<f32>,
    pub is_ghost_mode: bool,
    pub privacy_radius_meters: i32,
    pub updated_at: DateTime<Utc>,
    pub version: i32,
    pub distance: Option<f64>,
}

impl TryFrom<PostgresLocationRow> for UserLocation {
    type Error = DomainError;

    fn try_from(row: PostgresLocationRow) -> Result<Self> {
        // 1. Reconstruction des Metrics (Zéro validation, mapping direct)
        let metrics = row.accuracy_meters.map(|acc| {
            LocationMetrics::from_raw(
                LocationAccuracy::from_raw(acc),
                row.altitude.map(Altitude::from_raw)
            )
        });

        // 2. Reconstruction du Mouvement
        let movement = match (row.speed, row.heading) {
            (Some(s), Some(h)) => Some(MovementMetrics::from_raw(
                Speed::from_raw(s),
                Heading::from_raw(h)
            )),
            _ => None,
        };

        // 3. Utilisation du tunnel RESTORE (Chemin Elite)
        // On ne passe plus par .build() qui est réservé à la création de nouveaux points GPS.
        Ok(UserLocationBuilder::restore(
            AccountId::from_uuid(row.account_id),
            RegionCode::from_raw(row.region_code),
            GeoPoint::from_raw(row.lon, row.lat),
            metrics,
            movement,
            row.is_ghost_mode,
            row.privacy_radius_meters,
            row.updated_at,
            row.version,
        ))
    }
}

impl From<&UserLocation> for PostgresLocationRow {
    fn from(l: &UserLocation) -> Self {
        Self {
            account_id: l.account_id().as_uuid(),
            region_code: l.region_code().to_string(),
            lat: l.coordinates().lat(),
            lon: l.coordinates().lon(),
            accuracy_meters: l.metrics().as_ref().map(|m| m.accuracy().value()),
            altitude: l.metrics().as_ref().and_then(|m| m.altitude().map(|a| a.value())),
            heading: l.movement().as_ref().map(|m| m.heading().value()),
            speed: l.movement().as_ref().map(|m| m.speed().value()),
            is_ghost_mode: l.is_ghost_mode(),
            privacy_radius_meters: l.privacy_radius_meters(),
            updated_at: l.updated_at(),
            version: l.version(),
            distance: None,
        }
    }
}