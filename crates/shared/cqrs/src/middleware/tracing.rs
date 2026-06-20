use std::future::Future;

use tracing::Instrument;

use crate::command::bus::CommandBus;
use crate::command::command::Command;
use crate::envelope::Envelope;
use crate::error::CqrsError;
use crate::query::bus::QueryBus;
use crate::query::query::Query;

use super::layer::{CommandLayer, QueryLayer};

/// Middleware that creates an OpenTelemetry-compatible tracing span around
/// every `dispatch` call.
///
/// ## Integration with the telemetry crate
///
/// `TracingLayer` uses only the `tracing` façade (`tracing::info_span!`).
/// The span is automatically picked up by the global `TracerProvider`
/// installed by `telemetry::init()`, giving full OTel visibility without a
/// direct dependency on the `telemetry` crate.
///
/// ## Span fields
///
/// | Field              | Value                                             |
/// |--------------------|---------------------------------------------------|
/// | `otel.kind`        | `"INTERNAL"`                                      |
/// | `message.type`     | Fully qualified Rust type name of the command/query|
/// | `message.id`       | UUIDv7 of the envelope (`message_id`)             |
/// | `correlation.id`   | Propagated `correlation_id` from the envelope     |
///
/// The span is entered around the `dispatch` future so it is active across
/// every `await` point, including the handler invocation.
#[derive(Debug, Clone, Default)]
pub struct TracingLayer;

// ── Command side ──────────────────────────────────────────────────────────────

pub struct TracingCommandBus<S> {
    inner: S,
}

impl<S> CommandLayer<S> for TracingLayer {
    type Service = TracingCommandBus<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TracingCommandBus { inner }
    }
}

impl<S: CommandBus> CommandBus for TracingCommandBus<S> {
    fn dispatch<C: Command>(
        &self,
        envelope: Envelope<C>,
    ) -> impl Future<Output = Result<(), CqrsError>> + Send + '_ {
        let span = tracing::info_span!(
            "cqrs.command.dispatch",
            otel.kind = "INTERNAL",
            message.type = std::any::type_name::<C>(),
            message.id = %envelope.message_id,
            correlation.id = %envelope.correlation_id,
        );
        self.inner.dispatch(envelope).instrument(span)
    }
}

// ── Query side ────────────────────────────────────────────────────────────────

pub struct TracingQueryBus<S> {
    inner: S,
}

impl<S> QueryLayer<S> for TracingLayer {
    type Service = TracingQueryBus<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TracingQueryBus { inner }
    }
}

impl<S: QueryBus> QueryBus for TracingQueryBus<S> {
    fn dispatch<Q: Query>(
        &self,
        envelope: Envelope<Q>,
    ) -> impl Future<Output = Result<Q::Response, CqrsError>> + Send + '_ {
        let span = tracing::info_span!(
            "cqrs.query.dispatch",
            otel.kind = "INTERNAL",
            message.type = std::any::type_name::<Q>(),
            message.id = %envelope.message_id,
            correlation.id = %envelope.correlation_id,
        );
        self.inner.dispatch(envelope).instrument(span)
    }
}
