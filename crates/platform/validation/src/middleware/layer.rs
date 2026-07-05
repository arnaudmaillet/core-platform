//! Tower-inspired validation middleware for the CQRS command pipeline.
//!
//! [`ValidationLayer`] is a zero-size marker that produces a
//! [`ValidationCommandBus`] wrapping any inner [`CommandBus`]. On every
//! `dispatch` call it invokes [`Validate::validate`] on the command payload
//! before forwarding to the inner bus. A violation short-circuits execution
//! immediately, returning a [`CqrsError`] without touching the handler.
//!
//! ## Placement in the pipeline
//!
//! Place `ValidationLayer` as the **outermost** layer so invalid commands are
//! rejected before any tracing span or idempotency record is created:
//!
//! ```rust,ignore
//! let bus = MiddlewarePipeline::new(inner_bus)
//!     .layer(ValidationLayer)   // ← outermost: rejects first
//!     .layer(IdempotencyLayer::new(store))
//!     .layer(TracingLayer)
//!     .layer(LoggingLayer)
//!     .build();
//! ```
//!
//! ## Zero overhead for valid commands
//!
//! When `validate()` returns `Ok(())` the only cost is a single function call
//! that the optimiser can inline. No heap allocation, no type-map lookup, no
//! dynamic dispatch — `C: Command` implies `C: Validate` via the supertrait
//! chain, so the call is fully monomorphised.

use std::future::Future;

use cqrs::{Command, CommandBus, CommandLayer, CqrsError, Envelope};

use crate::error::validation_error::ValidationError;

// ── ValidationLayer ───────────────────────────────────────────────────────────

/// Zero-size [`CommandLayer`] that injects pre-dispatch input validation.
///
/// Constructed once at application startup and composed into the pipeline
/// via [`MiddlewarePipeline::layer`](cqrs::MiddlewarePipeline::layer).
#[derive(Debug, Clone, Copy, Default)]
pub struct ValidationLayer;

impl<S> CommandLayer<S> for ValidationLayer {
    type Service = ValidationCommandBus<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ValidationCommandBus { inner }
    }
}

// ── ValidationCommandBus ──────────────────────────────────────────────────────

/// [`CommandBus`] decorator that validates the command payload before
/// forwarding to the inner bus.
///
/// Do not construct directly; obtain it via
/// [`ValidationLayer`] in a [`MiddlewarePipeline`](cqrs::MiddlewarePipeline).
pub struct ValidationCommandBus<S> {
    inner: S,
}

impl<S: CommandBus> CommandBus for ValidationCommandBus<S> {
    // Explicit RPIT (not `async fn`) on purpose: it keeps the `+ Send` bound
    // visible at the impl and matches the trait's declared signature — the
    // crate-wide no-async_trait idiom.
    #[allow(clippy::manual_async_fn)]
    fn dispatch<C: Command>(
        &self,
        envelope: Envelope<C>,
    ) -> impl Future<Output = Result<(), CqrsError>> + Send + '_ {
        // `C: Command` implies `C: validate_core::Validate` via the supertrait
        // bound on `Command`. The call is statically dispatched — zero overhead.
        async move {
            if let Err(violations) = envelope.payload.validate() {
                let err = ValidationError::new(violations);
                tracing::debug!(
                    command.type = std::any::type_name::<C>(),
                    violation.count = err.violations().len(),
                    "command validation failed — dispatch short-circuited",
                );
                return Err(CqrsError::from_handler(err));
            }

            self.inner.dispatch(envelope).await
        }
    }
}
