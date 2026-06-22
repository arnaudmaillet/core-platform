use std::future::Future;

use crate::domain::value_object::{AudioId, AuthorId, PostId};
use crate::error::TimelineError;

pub struct AudioFeedRow {
    pub post_id:         PostId,
    pub author_id:       AuthorId,
    pub published_at_ms: i64,
}

pub trait AudioFeedRepository: Send + Sync + 'static {
    fn insert(
        &self,
        audio_id:        &AudioId,
        post_id:         &PostId,
        author_id:       &AuthorId,
        published_at_ms: i64,
    ) -> impl Future<Output = Result<(), TimelineError>> + Send;

    fn delete(
        &self,
        audio_id:        &AudioId,
        post_id:         &PostId,
        published_at_ms: i64,
    ) -> impl Future<Output = Result<(), TimelineError>> + Send;

    fn list(
        &self,
        audio_id:  &AudioId,
        before_ms: Option<i64>,
        limit:     i32,
    ) -> impl Future<Output = Result<Vec<AudioFeedRow>, TimelineError>> + Send;
}
