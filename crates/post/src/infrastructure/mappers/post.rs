// crates/post/src/infrastructure/mappers/post_row.rs

use chrono::{DateTime, Utc};
use std::collections::HashSet;
use std::str::FromStr;
use uuid::Uuid;

use infra_scylla::scylla;
use infra_scylla::scylla::value::CqlTimestamp;
use shared_kernel::core::{Error, Identifier, LifecycleTracker, Result, Versioned};
use shared_kernel::types::{MusicId, PostId, PostType, ProfileId};

use crate::entities::{MediaAsset, Post};
use crate::infrastructure::mappers::CqlMediaAssetRow;
use crate::types::{Caption, DynamicMetadata, Hashtags, Mentions, VisibilityLevel};

#[derive(Debug, scylla::DeserializeRow, Clone)]
pub struct CqlPostRow {
    pub author_id: Uuid,
    pub post_id: Uuid,
    pub post_type: String,
    pub caption: Option<String>,
    pub media_list: Vec<CqlMediaAssetRow>,
    pub total_duration_seconds: i32,
    pub allowed_comment_hands: bool,
    pub visibility_level: String,
    pub music_id: Option<Uuid>,
    pub hashtags: HashSet<String>,
    pub mentions: HashSet<Uuid>,
    pub version: i64,
    pub edited_at: Option<CqlTimestamp>,
    pub created_at: Option<CqlTimestamp>,
    pub updated_at: CqlTimestamp,
    pub dynamic_metadata: String,
}

impl CqlPostRow {
    pub fn from_domain(p: &Post) -> Self {
        let hashtags_set: HashSet<String> = p.hashtags().iter().map(|h| h.to_string()).collect();
        let mentions_set: HashSet<Uuid> = p.mentions().iter().map(|id| id.as_uuid()).collect();

        let cql_media: Vec<CqlMediaAssetRow> = p
            .media_list()
            .iter()
            .map(CqlMediaAssetRow::from_domain)
            .collect();

        Self {
            author_id: p.author_id().as_uuid(),
            post_id: p.post_id().as_uuid(),
            post_type: p.post_type().to_string(),
            caption: p.caption().as_ref().map(|c| c.to_string()),
            media_list: cql_media,
            total_duration_seconds: p.total_duration_seconds() as i32,
            allowed_comment_hands: p.allowed_comment_hands(),
            visibility_level: p.visibility_level().to_string(),
            music_id: p.music_id().map(|m| m.as_uuid()),
            hashtags: hashtags_set,
            mentions: mentions_set,
            version: p.version() as i64,
            edited_at: p.edited_at().map(|dt| CqlTimestamp(dt.timestamp_millis())),

            created_at: Some(CqlTimestamp(p.created_at().timestamp_millis())),

            updated_at: CqlTimestamp(p.updated_at().timestamp_millis()),
            dynamic_metadata: p.dynamic_metadata().to_string(),
        }
    }

    pub fn to_domain(self) -> Result<Post> {
        let post_id = PostId::from_uuid(self.post_id);
        let author_id = ProfileId::from_uuid(self.author_id);

        let edited_at = self
            .edited_at
            .and_then(|cql_ts| DateTime::<Utc>::from_timestamp_millis(cql_ts.0));

        let domain_created_at = self
            .created_at
            .and_then(|cql_ts| DateTime::<Utc>::from_timestamp_millis(cql_ts.0))
            .unwrap_or_else(|| post_id.created_at());

        let post_type = PostType::from_str(&self.post_type).map_err(|e| {
            Error::internal(format!("Invalid post_type '{}': {}", self.post_type, e))
        })?;

        let visibility_level = VisibilityLevel::from_str(&self.visibility_level).map_err(|e| {
            Error::internal(format!(
                "Invalid visibility_level '{}': {}",
                self.visibility_level, e
            ))
        })?;

        let domain_media: Vec<MediaAsset> = self
            .media_list
            .into_iter()
            .map(|cql| cql.to_domain())
            .collect::<Result<Vec<MediaAsset>>>()?;

        let caption = match self.caption {
            Some(text) if !text.trim().is_empty() => Some(Caption::try_new(text)?),
            _ => None,
        };

        let dynamic_metadata = if self.dynamic_metadata.trim().is_empty() {
            DynamicMetadata::empty()
        } else {
            DynamicMetadata::from_str(&self.dynamic_metadata)
                .map_err(|e| Error::internal(format!("Invalid dynamic_metadata JSON: {}", e)))?
        };

        let domain_mentions = Mentions::try_new(
            self.mentions
                .into_iter()
                .map(ProfileId::from_uuid)
                .collect(),
        )?;

        let domain_hashtags = Hashtags::try_from(self.hashtags.into_iter().collect::<Vec<_>>())?;
        let music_id = self.music_id.map(MusicId::from_uuid);

        let version_u64: u64 = self
            .version
            .try_into()
            .map_err(|_| Error::internal("Negative version detected in ScyllaDB for Post Shard"))?;

        let system_updated_at = DateTime::<Utc>::from_timestamp_millis(self.updated_at.0)
            .ok_or_else(|| Error::internal("Invalid updated_at timestamp"))?;

        Ok(Post::restore(
            post_id,
            author_id,
            post_type,
            caption,
            domain_media,
            self.total_duration_seconds as u32,
            self.allowed_comment_hands,
            visibility_level,
            music_id,
            domain_hashtags,
            domain_mentions,
            dynamic_metadata,
            domain_created_at,
            edited_at,
            LifecycleTracker::restore(system_updated_at),
            version_u64,
        ))
    }
}
