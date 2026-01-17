use chrono::{DateTime, Utc};
use shared_kernel::domain::events::{AggregateRoot, AggregateMetadata};
use shared_kernel::domain::entities::EntityMetadata;
use shared_kernel::domain::value_objects::{GeoPoint, RegionCode, AccountId};
use shared_kernel::errors::{DomainError, Result};
use crate::domain::events::LocationEvent;
use crate::domain::value_objects::{LocationMetrics, MovementMetrics};

#[derive(Debug, Clone)]
pub struct UserLocation {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub coordinates: GeoPoint,
    pub metrics: Option<LocationMetrics>,
    pub movement: Option<MovementMetrics>,

    // --- Privacy Settings ---
    pub is_ghost_mode: bool,
    pub privacy_radius_meters: i32,

    pub updated_at: DateTime<Utc>,
    pub metadata: AggregateMetadata,
}

impl UserLocation {
    /// Initialisation d'une nouvelle localisation
    pub fn new(account_id: AccountId, region: RegionCode, coords: GeoPoint) -> Self {
        Self {
            account_id: account_id,
            region_code: region,
            coordinates: coords,
            metrics: None,
            movement: None,
            is_ghost_mode: false,
            privacy_radius_meters: 0,
            updated_at: Utc::now(),
            metadata: AggregateMetadata::default(),
        }
    }

    /// Mise à jour de la position GPS avec les nouveaux VO
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

        // On émet l'événement pour que le reste du système (Feed, Maps) réagisse
        self.add_event(Box::new(LocationEvent::PositionUpdated {
            account_id: self.account_id.clone(),
            region: self.region_code.clone(),
            coordinates: self.coordinates,
            occurred_at: self.updated_at,
        }));
    }

    /// Activation/Désactivation du mode Fantôme
    pub fn set_ghost_mode(&mut self, enabled: bool) -> bool {
        if self.is_ghost_mode == enabled {
            return false;
        }
        self.is_ghost_mode = enabled;
        self.apply_change();

        self.add_event(Box::new(LocationEvent::LocationPrivacyChanged {
            account_id: self.account_id.clone(),
            is_ghost_mode: enabled,
            privacy_radius_meters: self.privacy_radius_meters,
            occurred_at: self.updated_at,
        }));
        true
    }

    /// Ajustement du rayon de confidentialité
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

impl EntityMetadata for UserLocation {
    fn entity_name() -> &'static str { "UserLocation" }
}

impl AggregateRoot for UserLocation {
    fn id(&self) -> String { self.account_id.to_string() }
    fn metadata(&self) -> &AggregateMetadata { &self.metadata }
    fn metadata_mut(&mut self) -> &mut AggregateMetadata { &mut self.metadata }
}