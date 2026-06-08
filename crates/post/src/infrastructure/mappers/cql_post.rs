// crates/post/src/infrastructure/mappers/post_row.rs

use chrono::{TimeZone, Utc};
use std::collections::HashSet;
use std::str::FromStr;
use uuid::Uuid;

use infra_scylla::scylla;
use infra_scylla::scylla::value::CqlTimestamp;
use infra_scylla::scylla_macros::DeserializeRow;
use shared_kernel::core::{Error, Identifier, LifecycleTracker, Result};
use shared_kernel::types::{MusicId, PostId, PostType, ProfileId};

use crate::entities::{MediaAsset, Post};
use crate::mappers::CqlMediaAsset;
use crate::types::{Caption, DynamicMetadata, Hashtags, Mentions, VisibilityLevel};

#[derive(Debug, DeserializeRow)]
pub struct CqlPostRow {
    pub region: String,
    pub author_id: Uuid,
    pub post_id: Uuid,
    pub post_type: String,
    pub caption: Option<String>,
    pub media_list: Vec<CqlMediaAsset>,
    pub total_duration_seconds: i32,
    pub allowed_comment_hands: bool,
    pub visibility_level: String,
    pub music_id: Option<Uuid>,
    pub hashtags: HashSet<String>,
    pub mentions: HashSet<Uuid>,
    pub edited_at: Option<CqlTimestamp>,
    pub updated_at: Option<CqlTimestamp>,
    pub dynamic_metadata: String,
}

impl TryFrom<CqlPostRow> for Post {
    type Error = Error;

    fn try_from(row: CqlPostRow) -> Result<Self> {
        let post_id = PostId::from_uuid(row.post_id);
        let author_id = ProfileId::from_uuid(row.author_id);

        let (secs, nanos) = row
            .post_id
            .get_timestamp()
            .map(|t| t.to_unix())
            .ok_or_else(|| Error::internal("Failed to extract created_at from Post UUIDv7"))?;
        let created_at = Utc
            .timestamp_opt(secs as i64, nanos as u32)
            .single()
            .ok_or_else(|| Error::internal("Invalid timestamp extracted from Post UUIDv7"))?;

        let system_updated_at = match row.updated_at {
            Some(cql_ts) => Utc
                .timestamp_millis_opt(cql_ts.0)
                .single()
                .unwrap_or(created_at),
            None => created_at,
        };

        let edited_at = match row.edited_at {
            Some(cql_ts) => Utc.timestamp_millis_opt(cql_ts.0).single(),
            None => None,
        };

        let post_type = PostType::from_str(&row.post_type).map_err(|e| {
            Error::internal(format!("Invalid post_type '{}': {}", row.post_type, e))
        })?;

        let visibility_level = VisibilityLevel::from_str(&row.visibility_level).map_err(|e| {
            Error::internal(format!(
                "Invalid visibility_level '{}': {}",
                row.visibility_level, e
            ))
        })?;

        let domain_media: Vec<MediaAsset> = row
            .media_list
            .into_iter()
            .map(MediaAsset::try_from)
            .collect::<Result<Vec<MediaAsset>>>()?;

        let caption = match row.caption {
            Some(text) if !text.trim().is_empty() => Some(Caption::try_new(text)?),
            _ => None,
        };

        let dynamic_metadata = if row.dynamic_metadata.trim().is_empty() {
            DynamicMetadata::empty()
        } else {
            DynamicMetadata::from_str(&row.dynamic_metadata)
                .map_err(|e| Error::internal(format!("Invalid dynamic_metadata JSON: {}", e)))?
        };

        let domain_mentions =
            Mentions::try_new(row.mentions.into_iter().map(ProfileId::from_uuid).collect())?;
        let domain_hashtags = Hashtags::try_from(row.hashtags.into_iter().collect::<Vec<_>>())?;

        let music_id = row.music_id.map(MusicId::from_uuid);

        Ok(Post::restore(
            post_id,
            author_id,
            post_type,
            caption,
            domain_media,
            row.total_duration_seconds as u32,
            row.allowed_comment_hands,
            visibility_level,
            music_id,
            domain_hashtags,
            domain_mentions,
            dynamic_metadata,
            edited_at,
            LifecycleTracker::restore(system_updated_at),
        ))
    }
}
