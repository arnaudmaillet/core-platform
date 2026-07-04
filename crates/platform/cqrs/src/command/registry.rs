use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use futures::future::BoxFuture;

use crate::envelope::Envelope;
use crate::error::CqrsError;

use super::bus::CommandBus;
use super::command::Command;
use super::handler::CommandHandler;

// ── Type-erased handler bridge ────────────────────────────────────────────────

/// Object-safe, type-erased version of [`CommandHandler`].
///
/// Stored in the registry as `Arc<dyn ErasedCommandHandler>`. The concrete
/// type is recovered at dispatch time via a `Box<dyn Any + Send>` downcast.
/// Sealed to this crate — external code never implements or names this trait.
pub(crate) trait ErasedCommandHandler: Send + Sync {
    /// Accepts an already-boxed `Envelope<C>` (erased as `dyn Any`) and
    /// dispatches it to the concrete handler, returning a boxed future.
    fn handle_erased<'a>(
        &'a self,
        envelope: Box<dyn Any + Send>,
    ) -> BoxFuture<'a, Result<(), CqrsError>>;
}

/// Bridges a concrete [`CommandHandler<C>`] to [`ErasedCommandHandler`].
///
/// Stores the handler behind an `Arc` so it can be cloned cheaply in the
/// `handle_erased` body without borrowing `self` into the returned future.
struct TypedHandlerBridge<H, C> {
    handler: Arc<H>,
    _marker: PhantomData<C>,
}

impl<H, C> ErasedCommandHandler for TypedHandlerBridge<H, C>
where
    H: CommandHandler<C>,
    C: Command,
{
    fn handle_erased<'a>(
        &'a self,
        envelope: Box<dyn Any + Send>,
    ) -> BoxFuture<'a, Result<(), CqrsError>> {
        let handler = Arc::clone(&self.handler);
        Box::pin(async move {
            // Safety: the registry only stores a TypedHandlerBridge<H, C>
            // under TypeId::of::<C>(), so this downcast is always correct.
            let typed = *envelope
                .downcast::<Envelope<C>>()
                .expect("cqrs invariant: TypeId key matches Envelope<C> — this is a bug");
            handler.handle(typed).await.map_err(CqrsError::from_handler)
        })
    }
}

// ── InMemoryCommandBus ────────────────────────────────────────────────────────

/// In-process [`CommandBus`] backed by a `TypeId`-keyed handler registry.
///
/// Built via [`CommandBusBuilder`]. After construction the registry is
/// immutable and wrapped in an `Arc`, making the bus cheap to clone and
/// safe to share across tasks.
///
/// ## Dispatch cost
///
/// One `HashMap::get` lookup (by `TypeId`) followed by a heap allocation
/// for the boxed `Envelope<C>` passed to the erased handler. The actual
/// handler invocation is statically dispatched inside the bridge closure.
#[derive(Clone)]
pub struct InMemoryCommandBus {
    handlers: Arc<HashMap<TypeId, Arc<dyn ErasedCommandHandler>>>,
}

impl CommandBus for InMemoryCommandBus {
    async fn dispatch<C: Command>(&self, envelope: Envelope<C>) -> Result<(), CqrsError> {
        let type_id = TypeId::of::<C>();
        let handler = self
            .handlers
            .get(&type_id)
            .ok_or(CqrsError::HandlerNotFound {
                type_name: std::any::type_name::<C>(),
            })?;
        let boxed = Box::new(envelope) as Box<dyn Any + Send>;
        handler.handle_erased(boxed).await
    }
}

// ── CommandBusBuilder ─────────────────────────────────────────────────────────

/// Fluent builder for [`InMemoryCommandBus`].
///
/// Call [`register`](CommandBusBuilder::register) once per command type, then
/// [`build`](CommandBusBuilder::build) to produce the immutable bus.
/// Duplicate registration returns [`CqrsError::DuplicateRegistration`]
/// immediately so misconfiguration is caught at startup.
///
/// ## Example
///
/// ```rust,ignore
/// let bus = CommandBusBuilder::new()
///     .register::<CreatePostCommand, _>(CreatePostHandler::new(repo))?
///     .register::<DeletePostCommand, _>(DeletePostHandler::new(repo))?
///     .build();
/// ```
#[derive(Default)]
pub struct CommandBusBuilder {
    handlers: HashMap<TypeId, Arc<dyn ErasedCommandHandler>>,
}

impl CommandBusBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `handler` as the single handler for command type `C`.
    ///
    /// Returns `Err(CqrsError::DuplicateRegistration)` if a handler for `C`
    /// was already registered.
    pub fn register<C, H>(mut self, handler: H) -> Result<Self, CqrsError>
    where
        C: Command,
        H: CommandHandler<C>,
    {
        let type_id = TypeId::of::<C>();
        if self.handlers.contains_key(&type_id) {
            return Err(CqrsError::DuplicateRegistration {
                type_name: std::any::type_name::<C>(),
            });
        }
        let bridge = Arc::new(TypedHandlerBridge {
            handler: Arc::new(handler),
            _marker: PhantomData::<C>,
        }) as Arc<dyn ErasedCommandHandler>;
        self.handlers.insert(type_id, bridge);
        Ok(self)
    }

    /// Consumes the builder and returns the immutable [`InMemoryCommandBus`].
    pub fn build(self) -> InMemoryCommandBus {
        InMemoryCommandBus {
            handlers: Arc::new(self.handlers),
        }
    }
}
