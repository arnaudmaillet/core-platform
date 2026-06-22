use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use cqrs::{Envelope, Query, QueryHandler};

use crate::application::port::{AudioFeedRepository, AudioFeedStore};
use crate::domain::value_object::AudioId;
use crate::error::TimelineError;

pub struct GetAudioFeedQuery {
    pub audio_id:   String,
    pub limit:      i32,
    pub page_token: String,
}

pub struct AudioFeedItem {
    pub post_id:         String,
    pub author_id:       String,
    pub published_at_ms: i64,
}

pub struct AudioFeedResponse {
    pub items:      Vec<AudioFeedItem>,
    pub next_token: String,
}

impl Query for GetAudioFeedQuery {
    type Response = AudioFeedResponse;
}

pub struct GetAudioFeedHandler<AFS, AFR> {
    pub audio_feed_store: Arc<AFS>,
    pub audio_feed_repo:  Arc<AFR>,
    pub max_page_size:    i32,
}

impl<AFS, AFR> QueryHandler<GetAudioFeedQuery>
    for GetAudioFeedHandler<AFS, AFR>
where
    AFS: AudioFeedStore,
    AFR: AudioFeedRepository,
{
    type Error = TimelineError;

    async fn handle(
        &self,
        envelope: Envelope<GetAudioFeedQuery>,
    ) -> Result<AudioFeedResponse, TimelineError> {
        let query = &envelope.payload;

        let audio_id  = AudioId::try_from(query.audio_id.as_str())?;
        let limit     = query.limit.min(self.max_page_size).max(1);
        let before_ms = decode_cursor(&query.page_token)?;

        let hot = self
            .audio_feed_store
            .range(&audio_id, before_ms, limit as u16)
            .await?;

        let members = if !hot.is_empty() {
            hot.into_iter()
                .map(|m| AudioFeedItem {
                    post_id:         m.post_id.to_string(),
                    author_id:       m.author_id.to_string(),
                    published_at_ms: m.published_at_ms,
                })
                .collect::<Vec<_>>()
        } else {
            let cold = self
                .audio_feed_repo
                .list(&audio_id, before_ms, limit)
                .await?;

            cold.into_iter()
                .map(|r| AudioFeedItem {
                    post_id:         r.post_id.to_string(),
                    author_id:       r.author_id.to_string(),
                    published_at_ms: r.published_at_ms,
                })
                .collect()
        };

        let next_token = if members.len() == limit as usize {
            members
                .last()
                .map(|last| encode_cursor(last.published_at_ms))
                .unwrap_or_default()
        } else {
            String::new()
        };

        Ok(AudioFeedResponse { items: members, next_token })
    }
}

fn encode_cursor(published_at_ms: i64) -> String {
    URL_SAFE_NO_PAD.encode(published_at_ms.to_string().as_bytes())
}

fn decode_cursor(token: &str) -> Result<Option<i64>, TimelineError> {
    if token.is_empty() {
        return Ok(None);
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(token)
        .map_err(|_| TimelineError::InvalidPageToken { token: token.to_owned() })?;
    let s = std::str::from_utf8(&bytes)
        .map_err(|_| TimelineError::InvalidPageToken { token: token.to_owned() })?;
    let ms = s.parse::<i64>()
        .map_err(|_| TimelineError::InvalidPageToken { token: token.to_owned() })?;
    Ok(Some(ms))
}
