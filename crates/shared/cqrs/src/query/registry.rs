use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use futures::future::BoxFuture;

use crate::envelope::Envelope;
use crate::error::CqrsError;

use super::bus::QueryBus;
use super::handler::QueryHandler;
use super::query::Query;

// ── Type-erased handler bridge ────────────────────────────────────────────────

/// Object-safe, type-erased version of [`QueryHandler`].
///
/// The response is boxed as `Box<dyn Any + Send>` and downcast back to
/// `Q::Response` in [`InMemoryQueryBus::dispatch`] after the future resolves.
pub(crate) trait ErasedQueryHandler: Send + Sync {
    fn handle_erased<'a>(
        &'a self,
        envelope: Box<dyn Any + Send>,
    ) -> BoxFuture<'a, Result<Box<dyn Any + Send>, CqrsError>>;
}

struct TypedQueryHandlerBridge<H, Q> {
    handler: Arc<H>,
    _marker: PhantomData<Q>,
}

impl<H, Q> ErasedQueryHandler for TypedQueryHandlerBridge<H, Q>
where
    H: QueryHandler<Q>,
    Q: Query,
{
    fn handle_erased<'a>(
        &'a self,
        envelope: Box<dyn Any + Send>,
    ) -> BoxFuture<'a, Result<Box<dyn Any + Send>, CqrsError>> {
        let handler = Arc::clone(&self.handler);
        Box::pin(async move {
            let typed = *envelope
                .downcast::<Envelope<Q>>()
                .expect("cqrs invariant: TypeId key matches Envelope<Q> — this is a bug");
            handler
                .handle(typed)
                .await
                .map(|r| Box::new(r) as Box<dyn Any + Send>)
                .map_err(CqrsError::from_handler)
        })
    }
}

// ── InMemoryQueryBus ──────────────────────────────────────────────────────────

/// In-process [`QueryBus`] backed by a `TypeId`-keyed handler registry.
///
/// Built via [`QueryBusBuilder`]. After construction the registry is
/// immutable and wrapped in an `Arc`, making the bus cheap to clone.
///
/// ## Dispatch cost
///
/// Same as [`InMemoryCommandBus`]: one `HashMap::get` plus one heap
/// allocation for the erased envelope, then two `downcast` calls
/// (one for the envelope, one for the response).
#[derive(Clone)]
pub struct InMemoryQueryBus {
    handlers: Arc<HashMap<TypeId, Arc<dyn ErasedQueryHandler>>>,
}

impl QueryBus for InMemoryQueryBus {
    async fn dispatch<Q: Query>(&self, envelope: Envelope<Q>) -> Result<Q::Response, CqrsError> {
        let type_id = TypeId::of::<Q>();
        let handler = self
            .handlers
            .get(&type_id)
            .ok_or(CqrsError::HandlerNotFound {
                type_name: std::any::type_name::<Q>(),
            })?;
        let boxed = Box::new(envelope) as Box<dyn Any + Send>;
        let result = handler.handle_erased(boxed).await?;
        // Safety: the bridge boxes `Q::Response` and we downcast back to the same type.
        Ok(*result
            .downcast::<Q::Response>()
            .expect("cqrs invariant: response type matches Q::Response — this is a bug"))
    }
}

// ── QueryBusBuilder ───────────────────────────────────────────────────────────

/// Fluent builder for [`InMemoryQueryBus`].
///
/// ## Example
///
/// ```rust
/// let bus = QueryBusBuilder::new()
///     .register::<GetPostByIdQuery, _>(GetPostByIdHandler::new(read_db))?
///     .register::<ListUserFeedQuery, _>(ListUserFeedHandler::new(read_db))?
///     .build();
/// ```
#[derive(Default)]
pub struct QueryBusBuilder {
    handlers: HashMap<TypeId, Arc<dyn ErasedQueryHandler>>,
}

impl QueryBusBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `handler` as the single handler for query type `Q`.
    pub fn register<Q, H>(mut self, handler: H) -> Result<Self, CqrsError>
    where
        Q: Query,
        H: QueryHandler<Q>,
    {
        let type_id = TypeId::of::<Q>();
        if self.handlers.contains_key(&type_id) {
            return Err(CqrsError::DuplicateRegistration {
                type_name: std::any::type_name::<Q>(),
            });
        }
        let bridge = Arc::new(TypedQueryHandlerBridge {
            handler: Arc::new(handler),
            _marker: PhantomData::<Q>,
        }) as Arc<dyn ErasedQueryHandler>;
        self.handlers.insert(type_id, bridge);
        Ok(self)
    }

    pub fn build(self) -> InMemoryQueryBus {
        InMemoryQueryBus {
            handlers: Arc::new(self.handlers),
        }
    }
}
