// crates/profile/src/infrastructure/repositories/rows/postgres_location_row.rs

use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use shared_kernel::domain::value_objects::{Altitude, GeoPoint, Heading, LocationAccuracy, RegionCode, Speed, AccountId};
use shared_kernel::errors::{Result, DomainError};
use crate::domain::entities::UserLocation;
use crate::domain::builders::UserLocationBuilder;
use crate::domain::value_objects::{LocationMetrics, MovementMetrics};

#[derive(FromRow)]
pub struct PostgresLocationRow {
    pub account_id: Uuid,
    pub region_code: String,
    pub lat: f64,
    pub lon: f64,
    pub accuracy_meters: Option<f32>,
    pub altitude: Option<f32>,
    pub heading: Option<f32>,
    pub speed: Option<f32>,
    pub is_ghost_mode: bool,
    pub privacy_radius_meters: i32,
    pub updated_at: DateTime<Utc>,
    pub version: i32, // Indispensable pour l'OCC
    // Optionnel : Utilisé uniquement lors des requêtes PostGIS de proximité
    pub distance: Option<f64>,
}

impl TryFrom<PostgresLocationRow> for UserLocation {
    type Error = DomainError;

    fn try_from(row: PostgresLocationRow) -> Result<Self> {
        // 1. Reconstruction des Metrics (Zéro validation, mapping direct)
        let metrics = row.accuracy_meters.map(|acc| {
            LocationMetrics::new_unchecked(
                LocationAccuracy::new_unchecked(acc),
                row.altitude.map(Altitude::new_unchecked)
            )
        });

        // 2. Reconstruction du Mouvement
        let movement = match (row.speed, row.heading) {
            (Some(s), Some(h)) => Some(MovementMetrics::new_unchecked(
                Speed::new_unchecked(s),
                Heading::new_unchecked(h)
            )),
            _ => None,
        };

        // 3. Utilisation du tunnel RESTORE (Chemin Elite)
        // On ne passe plus par .build() qui est réservé à la création de nouveaux points GPS.
        Ok(UserLocationBuilder::restore(
            AccountId::new_unchecked(row.account_id),
            RegionCode::new_unchecked(row.region_code),
            GeoPoint::new_unchecked(row.lat, row.lon),
            metrics,
            movement,
            row.is_ghost_mode,
            row.privacy_radius_meters,
            row.updated_at,
            row.version,
        ))
    }
}