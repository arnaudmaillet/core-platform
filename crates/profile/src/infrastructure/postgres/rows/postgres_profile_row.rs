use chrono::{DateTime, Utc};
use sqlx::FromRow;
use serde_json::Value as JsonValue;
use uuid::Uuid;
use shared_kernel::domain::value_objects::{Counter, LocationLabel, RegionCode, Url, AccountId, Username};
use shared_kernel::errors::{Result, DomainError};
use crate::domain::builders::ProfileBuilder;
use crate::domain::entities::Profile;
use crate::domain::value_objects::{Bio, DisplayName};

#[derive(FromRow)]
pub struct PostgresProfileRow {
    pub account_id: Uuid,
    pub region_code: String,
    pub display_name: String,
    pub username: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub banner_url: Option<String>,
    pub location_label: Option<String>,
    pub social_links: JsonValue,
    pub post_count: i64,
    pub is_private: bool,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TryFrom<PostgresProfileRow> for Profile {
    type Error = DomainError;

    fn try_from(row: PostgresProfileRow) -> Result<Self> {
        // Transformation des types techniques en Value Objects (Chemin rapide)
        let social_links = serde_json::from_value(row.social_links)
            .map_err(|e| DomainError::Validation {
                field: "social_links",
                reason: e.to_string()
            })?;

        // Reconstruction de l'agrégat via la méthode de restauration
        let profile = ProfileBuilder::restore(
            AccountId::new_unchecked(row.account_id),
            RegionCode::new_unchecked(row.region_code),
            DisplayName::new_unchecked(row.display_name),
            Username::new_unchecked(row.username),
            row.bio.map(Bio::new_unchecked),
            row.avatar_url.map(Url::new_unchecked),
            row.banner_url.map(Url::new_unchecked),
            row.location_label.map(LocationLabel::new_unchecked),
            social_links,
            Counter::try_from(row.post_count)?,
            row.is_private,
            row.version,
            row.created_at,
            row.updated_at,
        );

        Ok(profile)
    }
}