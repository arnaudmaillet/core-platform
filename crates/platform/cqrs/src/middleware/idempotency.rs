use std::future::Future;
use std::sync::Arc;

use dashmap::DashMap;
use uuid::Uuid;

use crate::command::bus::CommandBus;
use crate::command::command::Command;
use crate::envelope::Envelope;
use crate::error::CqrsError;

use super::layer::CommandLayer;

// ── IdempotencyStore ──────────────────────────────────────────────────────────

/// Pluggable backend for the [`IdempotencyLayer`] deduplication check.
///
/// Implement this trait to replace the default in-memory store with a Redis,
/// Postgres, or ScyllaDB-backed store for distributed deployments.
///
/// ## Semantics
///
/// `mark_processed` is called **only on success** (when the inner bus returns
/// `Ok(())`). This means a failed command can be retried — the idempotency
/// record is only committed once it has been successfully processed.
pub trait IdempotencyStore: Send + Sync + 'static {
    /// Returns `true` if `message_id` was already successfully processed.
    fn is_processed(
        &self,
        message_id: Uuid,
    ) -> impl Future<Output = bool> + Send + '_;

    /// Marks `message_id` as successfully processed.
    fn mark_processed(
        &self,
        message_id: Uuid,
    ) -> impl Future<Output = ()> + Send + '_;
}

// ── InMemoryIdempotencyStore ──────────────────────────────────────────────────

/// Lock-free in-process idempotency store backed by [`DashMap`].
///
/// Suitable for single-instance deployments and testing. For distributed
/// scenarios where multiple replicas run concurrently, replace with a
/// Redis-backed implementation using atomic SET NX / EXPIRE.
///
/// Note: the seen set grows unbounded. Add a TTL-based eviction layer when
/// deploying in long-running services.
#[derive(Debug, Default)]
pub struct InMemoryIdempotencyStore {
    seen: DashMap<Uuid, ()>,
}

impl InMemoryIdempotencyStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl IdempotencyStore for InMemoryIdempotencyStore {
    async fn is_processed(&self, message_id: Uuid) -> bool {
        self.seen.contains_key(&message_id)
    }

    async fn mark_processed(&self, message_id: Uuid) {
        self.seen.insert(message_id, ());
    }
}

// ── IdempotencyLayer ──────────────────────────────────────────────────────────

/// Command-only middleware that deduplicates dispatches by `envelope.message_id`.
///
/// Queries are naturally idempotent (read-only) and do not need this layer.
///
/// ## Algorithm
///
/// 1. Check `store.is_processed(message_id)`.
/// 2. If **already processed** → return `Ok(())` immediately without calling
///    the inner bus (transparent skip).
/// 3. If **not processed** → forward to the inner bus.
/// 4. On `Ok(())` → call `store.mark_processed(message_id)`.
/// 5. On `Err(_)` → do **not** mark (allow retry).
///
/// ## Example
///
/// ```rust,ignore
/// let bus = MiddlewarePipeline::new(inner_bus)
///     .layer(IdempotencyLayer::new(InMemoryIdempotencyStore::new()))
///     .layer(TracingLayer)
///     .build();
/// ```
pub struct IdempotencyLayer<Store> {
    store: Arc<Store>,
}

impl<Store: IdempotencyStore> IdempotencyLayer<Store> {
    pub fn new(store: Store) -> Self {
        Self {
            store: Arc::new(store),
        }
    }

    pub fn with_shared(store: Arc<Store>) -> Self {
        Self { store }
    }
}

impl<Store: IdempotencyStore> Clone for IdempotencyLayer<Store> {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
        }
    }
}

// ── IdempotencyCommandBus ─────────────────────────────────────────────────────

pub struct IdempotencyCommandBus<S, Store> {
    inner: S,
    store: Arc<Store>,
}

impl<S, Store: IdempotencyStore> CommandLayer<S> for IdempotencyLayer<Store> {
    type Service = IdempotencyCommandBus<S, Store>;

    fn layer(&self, inner: S) -> Self::Service {
        IdempotencyCommandBus {
            inner,
            store: Arc::clone(&self.store),
        }
    }
}

impl<S: CommandBus, Store: IdempotencyStore> CommandBus for IdempotencyCommandBus<S, Store> {
    fn dispatch<C: Command>(
        &self,
        envelope: Envelope<C>,
    ) -> impl Future<Output = Result<(), CqrsError>> + Send + '_ {
        let store = Arc::clone(&self.store);

        async move {
            let message_id = envelope.message_id;

            if store.is_processed(message_id).await {
                tracing::info!(
                    %message_id,
                    message.type = std::any::type_name::<C>(),
                    "idempotency: duplicate command skipped",
                );
                return Ok(());
            }

            let result = self.inner.dispatch(envelope).await;

            if result.is_ok() {
                store.mark_processed(message_id).await;
            }

            result
        }
    }
}
