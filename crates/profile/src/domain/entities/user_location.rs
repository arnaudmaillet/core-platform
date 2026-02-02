// crates/location/src/domain/entities/user_location.rs

use chrono::{DateTime, Utc};
use shared_kernel::domain::events::{AggregateRoot, AggregateMetadata};
use shared_kernel::domain::entities::{EntityMetadata, GeoPoint};
use shared_kernel::domain::Identifier;
use shared_kernel::domain::value_objects::{RegionCode, AccountId};
use shared_kernel::errors::{DomainError, Result};
use crate::domain::events::LocationEvent;
use crate::domain::value_objects::{LocationMetrics, MovementMetrics};

#[derive(Debug, Clone)]
pub struct UserLocation {
    account_id: AccountId,
    region_code: RegionCode,
    coordinates: GeoPoint,
    metrics: Option<LocationMetrics>,
    movement: Option<MovementMetrics>,
    is_ghost_mode: bool,
    privacy_radius_meters: i32,
    updated_at: DateTime<Utc>,
    metadata: AggregateMetadata,
}

impl UserLocation {
    pub(crate) fn new_from_builder(
        account_id: AccountId,
        region_code: RegionCode,
        coordinates: GeoPoint,
        metrics: Option<LocationMetrics>,
        movement: Option<MovementMetrics>,
        is_ghost_mode: bool,
        privacy_radius_meters: i32,
        updated_at: DateTime<Utc>,
        version: i32,
        is_restore: bool,
    ) -> Self {
        let metadata = if is_restore {
            AggregateMetadata::restore(version)
        } else {
            AggregateMetadata::new(version)
        };

        Self {
            account_id,
            region_code,
            coordinates,
            metrics,
            movement,
            is_ghost_mode,
            privacy_radius_meters,
            updated_at,
            metadata,
        }
    }

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
    ) -> Self {
        Self {
            account_id,
            region_code,
            coordinates,
            metrics,
            movement,
            is_ghost_mode,
            privacy_radius_meters,
            updated_at,
            metadata: AggregateMetadata::restore(version),
        }
    }

    // --- Getters (Lecture seule) ---

    pub fn account_id(&self) -> &AccountId { &self.account_id }
    pub fn region_code(&self) -> &RegionCode { &self.region_code }
    pub fn coordinates(&self) -> &GeoPoint { &self.coordinates }
    pub fn metrics(&self) -> Option<&LocationMetrics> { self.metrics.as_ref() }
    pub fn movement(&self) -> Option<&MovementMetrics> { self.movement.as_ref() }
    pub fn is_ghost_mode(&self) -> bool { self.is_ghost_mode }
    pub fn privacy_radius_meters(&self) -> i32 { self.privacy_radius_meters }
    pub fn updated_at(&self) -> DateTime<Utc> { self.updated_at }

    // --- Logic Métier (Commandes) ---

    pub fn update_position(
        &mut self,
        coords: GeoPoint,
        metrics: Option<LocationMetrics>,
        movement: Option<MovementMetrics>,
    ) {
        self.coordinates = coords;
        self.metrics = metrics;
        self.movement = movement;

        self.apply_change();

        self.add_event(Box::new(LocationEvent::PositionUpdated {
            account_id: self.account_id.clone(),
            region: self.region_code.clone(),
            coordinates: self.coordinates,
            occurred_at: self.updated_at,
        }));
    }

    pub fn set_ghost_mode(&mut self, enabled: bool) {
        if self.is_ghost_mode != enabled {
            self.is_ghost_mode = enabled;
            self.apply_change();

            self.add_event(Box::new(LocationEvent::LocationPrivacyChanged {
                account_id: self.account_id.clone(),
                region: self.region_code.clone(),
                is_ghost_mode: enabled,
                privacy_radius_meters: self.privacy_radius_meters,
                occurred_at: self.updated_at,
            }));
        }
    }

    pub fn update_privacy_radius(&mut self, radius_meters: i32) -> Result<()> {
        if !(0..=5000).contains(&radius_meters) {
            return Err(DomainError::Validation {
                field: "privacy_radius_meters",
                reason: "Radius must be between 0 and 5000 meters".to_string(),
            });
        }

        if self.privacy_radius_meters != radius_meters {
            self.privacy_radius_meters = radius_meters;
            self.apply_change();

            self.add_event(Box::new(LocationEvent::LocationPrivacyChanged {
                account_id: self.account_id.clone(),
                region: self.region_code.clone(),
                is_ghost_mode: self.is_ghost_mode,
                privacy_radius_meters: self.privacy_radius_meters,
                occurred_at: self.updated_at,
            }));
        }
        Ok(())
    }

    fn apply_change(&mut self) {
        self.increment_version();
        self.updated_at = Utc::now();
    }
}

// --- Trait Implementations ---

impl EntityMetadata for UserLocation {
    fn entity_name() -> &'static str { "UserLocation" }
}

impl AggregateRoot for UserLocation {
    fn id(&self) -> String { self.account_id.as_string() }
    fn metadata(&self) -> &AggregateMetadata { &self.metadata }
    fn metadata_mut(&mut self) -> &mut AggregateMetadata { &mut self.metadata }
}