use std::future::Future;

use crate::domain::value_object::{AudioId, AuthorId, PostId};
use crate::error::TimelineError;

pub struct AudioFeedMember {
    pub post_id:         PostId,
    pub author_id:       AuthorId,
    pub published_at_ms: i64,
}

pub trait AudioFeedStore: Send + Sync + 'static {
    fn push(
        &self,
        audio_id:        &AudioId,
        post_id:         &PostId,
        author_id:       &AuthorId,
        published_at_ms: i64,
        cap:             u16,
    ) -> impl Future<Output = Result<(), TimelineError>> + Send;

    fn remove(
        &self,
        audio_id:        &AudioId,
        post_id:         &PostId,
        published_at_ms: i64,
    ) -> impl Future<Output = Result<(), TimelineError>> + Send;

    fn range(
        &self,
        audio_id:  &AudioId,
        before_ms: Option<i64>,
        limit:     u16,
    ) -> impl Future<Output = Result<Vec<AudioFeedMember>, TimelineError>> + Send;
}
