/// Tower-inspired composition primitive for the **command** pipeline.
///
/// A `CommandLayer<S>` takes an inner service `S` (typically a [`CommandBus`]
/// implementation) and wraps it in a new type that applies cross-cutting
/// behaviour before or after calling `S::dispatch`.
///
/// ## Composing layers
///
/// Use [`MiddlewarePipeline`] to stack layers in declaration order:
///
/// ```rust
/// let bus = MiddlewarePipeline::new(inner_bus)
///     .layer(IdempotencyLayer::new(store))  // outermost — checked first
///     .layer(TracingLayer)
///     .layer(LoggingLayer)                  // innermost wrapper
///     .build();
/// ```
///
/// Each `.layer(l)` call produces `MiddlewarePipeline<L::Service>` so the
/// full type is statically known at the composition root, giving zero
/// dynamic-dispatch overhead on the hot path.
pub trait CommandLayer<S> {
    type Service;
    fn layer(&self, inner: S) -> Self::Service;
}

/// Tower-inspired composition primitive for the **query** pipeline.
///
/// Symmetric to [`CommandLayer`]: wraps an inner [`QueryBus`] implementation
/// with cross-cutting behaviour.
pub trait QueryLayer<S> {
    type Service;
    fn layer(&self, inner: S) -> Self::Service;
}
