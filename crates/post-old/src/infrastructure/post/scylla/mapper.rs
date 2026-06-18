// crates/post/src/infrastructure/post/scylla/mapper.rs

use crate::entities::{MediaAsset, Post};
use crate::infrastructure::media::ScyllaMediaModel;
use crate::infrastructure::post::ScyllaPostModel;
use crate::types::{Caption, DynamicMetadata, Hashtags, Mentions, VisibilityLevel};
use chrono::{DateTime, Utc};
use infra_scylla::scylla::value::CqlTimestamp;
use shared_kernel::core::{Error, Identifier, LifecycleTracker, Result, Versioned};
use shared_kernel::types::{MusicId, PostId, PostType, ProfileId};
use std::collections::HashSet;
use std::str::FromStr;
use uuid::Uuid;

/// Convertit le modèle Domaine en modèle de persistance ScyllaDB
impl From<&Post> for ScyllaPostModel {
    fn from(p: &Post) -> Self {
        let hashtags_set: HashSet<String> = p.hashtags().iter().map(|h| h.to_string()).collect();
        let mentions_set: HashSet<Uuid> = p.mentions().iter().map(|id| id.as_uuid()).collect();
        let cql_media: Vec<ScyllaMediaModel> = p.media_list().iter().map(Into::into).collect();

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
}

/// Convertit le modèle de persistance ScyllaDB en modèle Domaine
impl TryFrom<ScyllaPostModel> for Post {
    type Error = Error;

    fn try_from(row: ScyllaPostModel) -> Result<Self> {
        let post_id = PostId::from_uuid(row.post_id);
        let author_id = ProfileId::from_uuid(row.author_id);

        let edited_at = row
            .edited_at
            .and_then(|cql_ts| DateTime::<Utc>::from_timestamp_millis(cql_ts.0));

        let domain_created_at = row
            .created_at
            .and_then(|cql_ts| DateTime::<Utc>::from_timestamp_millis(cql_ts.0))
            .unwrap_or_else(|| post_id.created_at());

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

        let version_u64: u64 = row
            .version
            .try_into()
            .map_err(|_| Error::internal("Negative version detected in ScyllaDB for Post Shard"))?;

        let system_updated_at = DateTime::<Utc>::from_timestamp_millis(row.updated_at.0)
            .ok_or_else(|| Error::internal("Invalid updated_at timestamp"))?;

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
            domain_created_at,
            edited_at,
            LifecycleTracker::restore(system_updated_at),
            version_u64,
        ))
    }
}
