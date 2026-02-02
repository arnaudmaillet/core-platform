use chrono::{DateTime, Utc};
use shared_kernel::domain::entities::GeoPoint;
use shared_kernel::domain::value_objects::{RegionCode, AccountId};
use crate::domain::entities::UserLocation;
use crate::domain::value_objects::{LocationMetrics, MovementMetrics};

pub struct UserLocationBuilder {
    account_id: AccountId,
    region_code: RegionCode,
    coordinates: GeoPoint,
    metrics: Option<LocationMetrics>,
    movement: Option<MovementMetrics>,
    is_ghost_mode: bool,
    privacy_radius_meters: i32,
    updated_at: DateTime<Utc>,
    version: i32,
}

impl UserLocationBuilder {
    /// CHEMIN 1 : CRÉATION (Nouveau signal GPS reçu)
    pub fn new(account_id: AccountId, region_code: RegionCode, coordinates: GeoPoint) -> Self {
        Self {
            account_id,
            region_code,
            coordinates,
            metrics: None,
            movement: None,
            is_ghost_mode: false,
            privacy_radius_meters: 0,
            updated_at: Utc::now(),
            version: 1,
        }
    }

    /// CHEMIN 2 : RESTAURATION (Statique, retourne directement l'entité)
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        account_id: AccountId,
        region_code: RegionCode,
        coordinates: GeoPoint,
        metrics: Option<LocationMetrics>,
        movement: Option<MovementMetrics>,
        is_ghost_mode: bool,
        privacy_radius_meters: i32,
        updated_at: DateTime<Utc>,
        version: i32,
    ) -> UserLocation {
        UserLocation::restore(
            account_id,
            region_code,
            coordinates,
            metrics,
            movement,
            is_ghost_mode,
            privacy_radius_meters,
            updated_at,
            version,
        )
    }

    // --- SETTERS ---

    pub fn with_metrics(mut self, metrics: Option<LocationMetrics>) -> Self {
        self.metrics = metrics;
        self
    }

    pub fn with_movement(mut self, movement: Option<MovementMetrics>) -> Self {
        self.movement = movement;
        self
    }

    pub fn with_privacy(mut self, is_ghost_mode: bool, privacy_radius_meters: i32) -> Self {
        self.is_ghost_mode = is_ghost_mode;
        self.privacy_radius_meters = privacy_radius_meters;
        self
    }

    pub fn build(self) -> UserLocation {
        UserLocation::new_from_builder(
            self.account_id,
            self.region_code,
            self.coordinates,
            self.metrics,
            self.movement,
            self.is_ghost_mode,
            self.privacy_radius_meters,
            self.updated_at,
            self.version,
            false,
        )
    }
}