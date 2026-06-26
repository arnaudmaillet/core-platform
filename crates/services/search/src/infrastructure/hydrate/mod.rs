//! Content hydration — the seam between a thin notification and a fat document.
//!
//! `post.v1.events` are notifications (ids + timestamps), so a publish/update can't
//! be projected directly: the consumer first resolves the authoritative snapshot
//! from the `post` service over gRPC. This runs on the **ingestion** path only; the
//! query path stays self-contained.

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use post_api::post_service_client::PostServiceClient;
use post_api::GetPostRequest;
use tonic::transport::Channel;
use tonic::Code;

use crate::domain::{EntityKind, EntityDeletion, PostEvent, PostSnapshot, SourceEvent};
use crate::error::SearchError;
use crate::infrastructure::decode::ContentRef;

/// Turns a [`ContentRef`] (from the decode layer) into a projectable
/// [`SourceEvent`] by fetching the source-of-record snapshot.
#[async_trait]
pub trait SourceHydrator: Send + Sync + 'static {
    async fn hydrate(
        &self,
        content_ref: ContentRef,
        now: DateTime<Utc>,
    ) -> Result<SourceEvent, SearchError>;
}

/// The production hydrator: a `post` gRPC client.
pub struct GrpcPostHydrator {
    client: PostServiceClient<Channel>,
}

impl GrpcPostHydrator {
    pub fn new(channel: Channel) -> Self {
        Self {
            client: PostServiceClient::new(channel),
        }
    }
}

#[async_trait]
impl SourceHydrator for GrpcPostHydrator {
    async fn hydrate(
        &self,
        content_ref: ContentRef,
        _now: DateTime<Utc>,
    ) -> Result<SourceEvent, SearchError> {
        match content_ref.kind {
            EntityKind::Post => {
                let mut client = self.client.clone();
                let view = match client
                    .get_post(GetPostRequest {
                        post_id: content_ref.id.clone(),
                    })
                    .await
                {
                    Ok(resp) => resp.into_inner(),
                    // The post was deleted between the event and our fetch — converge
                    // by removing it from the index (a delete is idempotent).
                    Err(status) if status.code() == Code::NotFound => {
                        return Ok(SourceEvent::Post(PostEvent::Deleted(EntityDeletion {
                            id: content_ref.id,
                        })));
                    }
                    // Transient source-service faults are retryable; the consumer
                    // retries then dead-letters rather than dropping the update.
                    Err(status) if is_transient(status.code()) => {
                        return Err(SearchError::EngineUnavailable);
                    }
                    Err(status) => {
                        return Err(SearchError::UnmappedSource {
                            reason: format!("get_post failed: {status}"),
                        });
                    }
                };

                let snapshot = PostSnapshot {
                    post_id: view.post_id,
                    // The post's author is its profile; `author_handle` is a display
                    // field left to a future secondary profile lookup.
                    author_id: view.profile_id,
                    author_handle: String::new(),
                    hashtags: extract_hashtags(&view.caption),
                    caption: view.caption,
                    // Thumbnail derivation from `attachments` is deferred (display-only).
                    thumbnail_key: String::new(),
                    created_at: ms_to_dt(view.created_at_ms),
                    revision: content_ref.revision,
                };
                Ok(SourceEvent::Post(PostEvent::Published(snapshot)))
            }
            // Only post content is hydrated today; profile ingestion is an upstream
            // prerequisite (profile publishes no Kafka stream yet).
            other => Err(SearchError::UnmappedSource {
                reason: format!("no hydrator for {other:?}"),
            }),
        }
    }
}

/// Extract `#hashtags` from caption text (a search-side responsibility — posts do
/// not model hashtags). Normalized to lowercase, de-duplicated.
fn extract_hashtags(caption: &str) -> Vec<String> {
    let mut tags: Vec<String> = caption
        .split_whitespace()
        .filter_map(|token| token.strip_prefix('#'))
        .map(|tag| {
            tag.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|tag| !tag.is_empty())
        .collect();
    tags.sort();
    tags.dedup();
    tags
}

fn ms_to_dt(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap_or_else(Utc::now)
}

fn is_transient(code: Code) -> bool {
    matches!(
        code,
        Code::Unavailable | Code::DeadlineExceeded | Code::ResourceExhausted | Code::Aborted
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_and_normalizes_hashtags() {
        let tags = extract_hashtags("Loving #Rust and #rust and #OpenSearch! no_tag");
        assert_eq!(tags, vec!["opensearch".to_owned(), "rust".to_owned()]);
    }

    #[test]
    fn no_hashtags_is_empty() {
        assert!(extract_hashtags("just a plain caption").is_empty());
    }
}
