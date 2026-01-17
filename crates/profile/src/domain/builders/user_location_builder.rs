use chrono::{DateTime, Utc};
use shared_kernel::domain::events::AggregateMetadata;
use shared_kernel::domain::value_objects::{GeoPoint, RegionCode, AccountId};
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
    /// Utilisé par les services/use cases. Initialise avec version 1 et NOW.
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

    /// CHEMIN 2 : RESTAURATION (Depuis la base de données)
    /// Utilisé exclusivement par les Repositories.
    /// Injection directe du AggregateMetadata::new_restored.
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
        UserLocation {
            account_id: account_id,
            region_code,
            coordinates,
            metrics,
            movement,
            is_ghost_mode,
            privacy_radius_meters,
            updated_at,
            // On restaure l'état technique sans lever d'événements
            metadata: AggregateMetadata::restore(version),
        }
    }

    // --- SETTERS (Chemin Création) ---

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

    /// Finalise pour une CRÉATION
    pub fn build(self) -> UserLocation {
        UserLocation {
            account_id: self.account_id,
            region_code: self.region_code,
            coordinates: self.coordinates,
            metrics: self.metrics,
            movement: self.movement,
            is_ghost_mode: self.is_ghost_mode,
            privacy_radius_meters: self.privacy_radius_meters,
            updated_at: self.updated_at,
            // Nouvelle instance avec versioning activé
            metadata: AggregateMetadata::new(self.version),
        }
    }
}