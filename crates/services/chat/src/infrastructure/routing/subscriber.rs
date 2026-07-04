use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use fred::interfaces::{EventInterface, PubsubInterface};
use redis_storage::RedisSubscriber;
use tokio::sync::broadcast::error::RecvError;
use tokio::task::JoinHandle;

use crate::domain::value_object::ConversationId;
use crate::error::ChatError;
use crate::infrastructure::cache::redis_err;
use crate::infrastructure::routing::channel::{ChannelScheme, Plane};
use crate::infrastructure::routing::plane::PlaneEvent;

/// Where a decoded inbound plane event is delivered locally — implemented by the
/// in-process gRPC broadcast registry. Defined here so the subscriber's run loop
/// stays decoupled from the streaming layer.
pub trait InboundSink: Send + Sync + 'static {
    fn deliver(&self, conversation_id: ConversationId, plane: Plane, event: PlaneEvent);
}

/// Per-pod attach/detach surface, refcounted per channel. Each method returns
/// `true` when it caused a real Redis `SSUBSCRIBE`/`SUNSUBSCRIBE` (first/last
/// local interest), so the caller can pair audience attaches with routing-
/// registry shard activation.
#[async_trait]
pub trait PlaneAttach: Send + Sync + 'static {
    async fn attach_member(&self, conversation_id: &ConversationId) -> Result<bool, ChatError>;
    async fn detach_member(&self, conversation_id: &ConversationId) -> Result<bool, ChatError>;
    async fn attach_audience(&self, conversation_id: &ConversationId, shard: u16)
        -> Result<bool, ChatError>;
    async fn detach_audience(&self, conversation_id: &ConversationId, shard: u16)
        -> Result<bool, ChatError>;
}

/// Per-pod subscription manager over a fred [`RedisSubscriber`].
///
/// Subscriptions are reference-counted per channel: the pod `SSUBSCRIBE`s a
/// channel on the **first** local stream that needs it and `SUNSUBSCRIBE`s on the
/// **last** one to leave (the reaper-on-zero). This bounds the pod's Redis
/// subscriptions to live `(pod x conversation)` pairs rather than to the number
/// of connected streams. A pod that hosts only guests never subscribes to a
/// member channel, so presence/typing/receipt traffic never reaches it.
pub struct PlaneSubscriber<S: InboundSink> {
    subscriber: RedisSubscriber,
    refcounts:  DashMap<String, usize>,
    sink:       Arc<S>,
}

impl<S: InboundSink> PlaneSubscriber<S> {
    pub fn new(subscriber: RedisSubscriber, sink: Arc<S>) -> Self {
        Self { subscriber, refcounts: DashMap::new(), sink }
    }

    async fn attach(&self, channel: String) -> Result<bool, ChatError> {
        // Bump the refcount under the shard lock, then subscribe outside it.
        let first = {
            let mut count = self.refcounts.entry(channel.clone()).or_insert(0);
            *count += 1;
            *count == 1
        };
        if first {
            self.subscriber.inner.ssubscribe(channel).await.map_err(redis_err)?;
        }
        Ok(first)
    }

    async fn detach(&self, channel: String) -> Result<bool, ChatError> {
        let reached_zero = match self.refcounts.get_mut(&channel) {
            Some(mut count) => {
                *count = count.saturating_sub(1);
                *count == 0
            }
            None => false,
        };
        // Re-check under the shard lock so a concurrent attach is not torn down.
        if reached_zero && self.refcounts.remove_if(&channel, |_, c| *c == 0).is_some() {
            self.subscriber.inner.sunsubscribe(channel).await.map_err(redis_err)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Spawns the receive loop: decode each inbound message and hand it to the
    /// [`InboundSink`]. Run once per pod after construction.
    pub fn spawn(self: Arc<Self>) -> JoinHandle<()> {
        let mut rx = self.subscriber.inner.message_rx();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(msg) => {
                        let channel: &str = &msg.channel;
                        let Some((conversation_id, plane)) = ChannelScheme::parse(channel) else {
                            continue;
                        };
                        let Some(payload) = msg.value.as_string() else { continue };
                        match PlaneEvent::from_json(&payload) {
                            Ok(event) => self.sink.deliver(conversation_id, plane, event),
                            Err(e) => tracing::warn!(
                                error = %e,
                                channel,
                                "failed to decode inbound plane event"
                            ),
                        }
                    }
                    Err(RecvError::Lagged(n)) => {
                        tracing::warn!(skipped = n, "plane subscriber lagged");
                    }
                    Err(RecvError::Closed) => break,
                }
            }
        })
    }
}

#[async_trait]
impl<S: InboundSink> PlaneAttach for PlaneSubscriber<S> {
    async fn attach_member(&self, conversation_id: &ConversationId) -> Result<bool, ChatError> {
        self.attach(ChannelScheme::member_channel(conversation_id)).await
    }

    async fn detach_member(&self, conversation_id: &ConversationId) -> Result<bool, ChatError> {
        self.detach(ChannelScheme::member_channel(conversation_id)).await
    }

    async fn attach_audience(
        &self,
        conversation_id: &ConversationId,
        shard:           u16,
    ) -> Result<bool, ChatError> {
        self.attach(ChannelScheme::audience_channel(conversation_id, shard)).await
    }

    async fn detach_audience(
        &self,
        conversation_id: &ConversationId,
        shard:           u16,
    ) -> Result<bool, ChatError> {
        self.detach(ChannelScheme::audience_channel(conversation_id, shard)).await
    }
}
