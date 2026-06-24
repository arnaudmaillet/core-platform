use std::future::Future;
use std::time::Instant;

use ::error::AppError;

use crate::command::bus::CommandBus;
use crate::command::command::Command;
use crate::envelope::Envelope;
use crate::error::CqrsError;
use crate::query::bus::QueryBus;
use crate::query::query::Query;

use super::layer::{CommandLayer, QueryLayer};

/// Middleware that emits structured log events before and after each dispatch.
///
/// Emits `tracing::info!` on dispatch start, and `tracing::info!` /
/// `tracing::error!` on completion with elapsed time. These events are
/// automatically captured by the global subscriber installed by
/// `telemetry::init()`.
///
/// ## Log fields
///
/// **Start event**
/// - `message.type` — fully qualified Rust type name
/// - `correlation.id` — envelope's `correlation_id`
/// - `message.id` — envelope's `message_id`
///
/// **Completion event**
/// - All start fields
/// - `elapsed_ms` — handler wall-clock time in milliseconds
/// - `error` / `error.code` — populated only on failure
#[derive(Debug, Clone, Default)]
pub struct LoggingLayer;

// ── Command side ──────────────────────────────────────────────────────────────

pub struct LoggingCommandBus<S> {
    inner: S,
}

impl<S> CommandLayer<S> for LoggingLayer {
    type Service = LoggingCommandBus<S>;

    fn layer(&self, inner: S) -> Self::Service {
        LoggingCommandBus { inner }
    }
}

impl<S: CommandBus> CommandBus for LoggingCommandBus<S> {
    fn dispatch<C: Command>(
        &self,
        envelope: Envelope<C>,
    ) -> impl Future<Output = Result<(), CqrsError>> + Send + '_ {
        let type_name = std::any::type_name::<C>();
        let correlation_id = envelope.correlation_id;
        let message_id = envelope.message_id;

        async move {
            tracing::info!(
                message.type = type_name,
                %correlation_id,
                %message_id,
                "command dispatch started",
            );

            let started = Instant::now();
            let result = self.inner.dispatch(envelope).await;
            let elapsed_ms = started.elapsed().as_millis();

            match &result {
                Ok(()) => tracing::info!(
                    message.type = type_name,
                    %correlation_id,
                    %message_id,
                    elapsed_ms,
                    "command dispatch completed",
                ),
                Err(e) => tracing::error!(
                    message.type = type_name,
                    %correlation_id,
                    %message_id,
                    elapsed_ms,
                    error = %e,
                    error.code = e.error_code(),
                    "command dispatch failed",
                ),
            }

            result
        }
    }
}

// ── Query side ────────────────────────────────────────────────────────────────

pub struct LoggingQueryBus<S> {
    inner: S,
}

impl<S> QueryLayer<S> for LoggingLayer {
    type Service = LoggingQueryBus<S>;

    fn layer(&self, inner: S) -> Self::Service {
        LoggingQueryBus { inner }
    }
}

impl<S: QueryBus> QueryBus for LoggingQueryBus<S> {
    fn dispatch<Q: Query>(
        &self,
        envelope: Envelope<Q>,
    ) -> impl Future<Output = Result<Q::Response, CqrsError>> + Send + '_ {
        let type_name = std::any::type_name::<Q>();
        let correlation_id = envelope.correlation_id;
        let message_id = envelope.message_id;

        async move {
            tracing::info!(
                message.type = type_name,
                %correlation_id,
                %message_id,
                "query dispatch started",
            );

            let started = Instant::now();
            let result = self.inner.dispatch(envelope).await;
            let elapsed_ms = started.elapsed().as_millis();

            match &result {
                Ok(_) => tracing::info!(
                    message.type = type_name,
                    %correlation_id,
                    %message_id,
                    elapsed_ms,
                    "query dispatch completed",
                ),
                Err(e) => tracing::error!(
                    message.type = type_name,
                    %correlation_id,
                    %message_id,
                    elapsed_ms,
                    error = %e,
                    error.code = e.error_code(),
                    "query dispatch failed",
                ),
            }

            result
        }
    }
}
