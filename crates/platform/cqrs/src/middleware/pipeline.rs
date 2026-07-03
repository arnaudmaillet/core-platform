use super::layer::{CommandLayer, QueryLayer};

/// Generic middleware composition builder.
///
/// Wraps an inner service `S` and allows stacking [`CommandLayer`] or
/// [`QueryLayer`] implementations around it. After all layers have been
/// added, call [`build`](MiddlewarePipeline::build) to extract the fully
/// decorated service.
///
/// ## Layer ordering
///
/// Layers are applied inside-out: the **first** `.layer()` call produces the
/// outermost wrapper (i.e. the first to execute on each `dispatch` call).
///
/// ```text
/// MiddlewarePipeline::new(bus)
///     .layer(IdempotencyLayer)   ← outermost: runs first, can short-circuit
///     .layer(TracingLayer)       ← creates span around inner layers + handler
///     .layer(LoggingLayer)       ← innermost wrapper: runs just before handler
///     .build()
/// ```
///
/// ## Zero dynamic dispatch
///
/// Each `.layer()` call is a type-level transformation. The final type
/// encodes the full stack:
/// `LoggingCommandBus<TracingCommandBus<IdempotencyCommandBus<InMemoryCommandBus>>>`
/// All dispatch calls are statically dispatched — no `dyn` allocation.
///
/// ## Usage with command pipelines
///
/// ```rust,ignore
/// let bus: LoggingCommandBus<TracingCommandBus<InMemoryCommandBus>> =
///     MiddlewarePipeline::new(inner_bus)
///         .layer(TracingLayer)
///         .layer(LoggingLayer)
///         .build();
/// ```
///
/// ## Usage with query pipelines
///
/// ```rust,ignore
/// let qbus = MiddlewarePipeline::new(inner_query_bus)
///     .query_layer(TracingLayer)
///     .build();
/// ```
#[derive(Clone)]
pub struct MiddlewarePipeline<S> {
    service: S,
}

impl<S> MiddlewarePipeline<S> {
    pub fn new(service: S) -> Self {
        Self { service }
    }

    /// Wraps the current service with a [`CommandLayer`].
    pub fn layer<L: CommandLayer<S>>(self, layer: L) -> MiddlewarePipeline<L::Service> {
        MiddlewarePipeline {
            service: layer.layer(self.service),
        }
    }

    /// Wraps the current service with a [`QueryLayer`].
    pub fn query_layer<L: QueryLayer<S>>(self, layer: L) -> MiddlewarePipeline<L::Service> {
        MiddlewarePipeline {
            service: layer.layer(self.service),
        }
    }

    /// Consumes the pipeline and returns the decorated service.
    pub fn build(self) -> S {
        self.service
    }
}
