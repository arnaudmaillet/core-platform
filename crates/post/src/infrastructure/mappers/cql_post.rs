// crates/post/src/infrastructure/mappers/post_row.rs

use chrono::{DateTime, Utc};
use std::collections::HashSet;
use std::str::FromStr;
use uuid::Uuid;

use infra_scylla::scylla;
use infra_scylla::scylla::value::CqlTimestamp;
use infra_scylla::scylla_macros::DeserializeRow;
use shared_kernel::core::{Error, Identifier, Result};
use shared_kernel::types::{MusicId, PostId, PostType, ProfileId};

use crate::domain::entities::MediaAsset;
use crate::domain::entities::Post;
use crate::domain::types::{
    Caption, DynamicMetadata, Hashtags, Mentions, VisibilityLevel,
}; // Ajout des types domaine
use crate::mappers::CqlMediaAsset;

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
    pub is_edited: bool,
    pub updated_at: Option<CqlTimestamp>,
    pub dynamic_metadata: String,
}

impl TryFrom<CqlPostRow> for Post {
    type Error = Error;

    fn try_from(row: CqlPostRow) -> Result<Self> {
        let post_id = PostId::from_uuid(row.post_id);
        let author_id = ProfileId::from_uuid(row.author_id);

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
        let domain_updated_at = row
            .updated_at
            .map(|cql_ts| DateTime::<Utc>::from_timestamp_millis(cql_ts.0))
            .flatten();

        let domain_mentions = Mentions::try_new(
            row.mentions
                .into_iter()
                .map(|u| ProfileId::from_uuid(u))
                .collect(),
        )?;
        let domain_hashtags = Hashtags::try_from(row.hashtags.into_iter().collect::<Vec<_>>())?;

        let mut builder = Post::builder(post_id, author_id, post_type, visibility_level)
            .with_media_list(domain_media)
            .with_optional_caption(caption)
            .with_comment_settings(row.allowed_comment_hands)
            .with_edit_status(row.is_edited, domain_updated_at)
            .with_dynamic_metadata(dynamic_metadata)
            .with_mentions(domain_mentions)
            .with_hashtags(domain_hashtags);

        if let Some(m_uuid) = row.music_id {
            builder = builder.with_music_id(MusicId::from_uuid(m_uuid));
        }

        builder.build()
    }
}
