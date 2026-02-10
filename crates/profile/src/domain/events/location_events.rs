use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use shared_kernel::domain::entities::GeoPoint;
use shared_kernel::domain::events::DomainEvent;
use shared_kernel::domain::value_objects::RegionCode;
use std::borrow::Cow;
use crate::domain::value_objects::ProfileId;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum LocationEvent {
    /// Mise à jour de la position GPS (Fréquent)
    PositionUpdated {
        profile_id: ProfileId,
        region: RegionCode,
        coordinates: GeoPoint,
        occurred_at: DateTime<Utc>,
    },

    /// Changement des paramètres de confidentialité
    LocationPrivacyChanged {
        profile_id: ProfileId,
        region: RegionCode,
        is_ghost_mode: bool,
        privacy_radius_meters: i32,
        occurred_at: DateTime<Utc>,
    },

    /// Signalement de sortie de zone (Geofencing)
    LeftZone {
        profile_id: ProfileId,
        region: RegionCode,
        zone_id: String,
        occurred_at: DateTime<Utc>,
    },
}

impl DomainEvent for LocationEvent {
    fn event_type(&self) -> Cow<'_, str> {
        match self {
            Self::PositionUpdated { .. } => Cow::Borrowed("location.updated"),
            Self::LocationPrivacyChanged { .. } => Cow::Borrowed("location.privacy.changed"),
            Self::LeftZone { .. } => Cow::Borrowed("location.zone.left"),
        }
    }

    fn aggregate_type(&self) -> Cow<'_, str> {
        Cow::Borrowed("location") // Agrégat distinct !
    }

    fn aggregate_id(&self) -> String {
        match self {
            Self::PositionUpdated { profile_id, .. }
            | Self::LocationPrivacyChanged { profile_id, .. }
            | Self::LeftZone { profile_id, .. } => profile_id.to_string(),
        }
    }

    fn region_code(&self) -> RegionCode {
        match self {
            Self::PositionUpdated { region, .. }
            | Self::LocationPrivacyChanged { region, .. }
            | Self::LeftZone { region, .. } => region.clone(),
        }
    }
    fn occurred_at(&self) -> DateTime<Utc> {
        match self {
            Self::PositionUpdated { occurred_at, .. }
            | Self::LocationPrivacyChanged { occurred_at, .. }
            | Self::LeftZone { occurred_at, .. } => *occurred_at,
        }
    }

    fn payload(&self) -> Value {
        json!(self)
    }
}
