//! Distributed error context and the [`DistributedError`] envelope.
//!
//! [`ErrorContext`] carries the "where / when / which request" metadata that
//! turns a bare error into something traceable across a fleet of services
//! (request id, OpenTelemetry trace/span ids, emitting service, timestamp,
//! arbitrary metadata). [`DistributedError`] then wraps a concrete service
//! error *together with* that context — keeping the error's real type (no
//! `Box<dyn Error>`) while making it loggable and convertible to an API
//! response in one place.

use std::collections::HashMap;
use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::Level;
use uuid::Uuid;

use crate::traits::AppError;

/// Cross-service request/trace metadata attached to every error.
///
/// Construct it once per inbound request (typically in middleware) with
/// [`ErrorContext::new`], enrich it with trace ids and metadata, then thread it
/// through so any error raised downstream can be reported with full context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    /// Unique id for this request. Generated automatically by
    /// [`ErrorContext::new`].
    pub request_id: Uuid,
    /// Distributed trace id (OpenTelemetry / Jaeger), if tracing is active.
    pub trace_id: Option<String>,
    /// Span id within the trace, if available.
    pub span_id: Option<String>,
    /// Identifier of the microservice that produced the error.
    pub service_name: &'static str,
    /// When the context was created (UTC).
    pub timestamp: DateTime<Utc>,
    /// Arbitrary structured fields (user id, route, tenant, ...). Exposed to
    /// clients as the `details` map of the API response.
    pub metadata: HashMap<String, String>,
}

impl ErrorContext {
    /// Creates a fresh context for `service_name`, generating a new
    /// `request_id` and stamping the current time.
    pub fn new(service_name: &'static str) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            trace_id: None,
            span_id: None,
            service_name,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Attaches distributed tracing identifiers. Builder-style; consumes and
    /// returns `self`.
    pub fn with_trace(mut self, trace_id: impl Into<String>, span_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self.span_id = Some(span_id.into());
        self
    }

    /// Adds a single metadata entry. Chainable.
    pub fn with_meta(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// A concrete service error `E` paired with its [`ErrorContext`].
///
/// Generic over `E` so the concrete error type is preserved end-to-end (no
/// type erasure), which keeps pattern-matching, [`AppError`] dispatch and
/// `source()` chaining fully static.
#[derive(Debug)]
pub struct DistributedError<E: AppError> {
    /// The concrete error raised by the service.
    pub error: E,
    /// The distributed context in which it occurred.
    pub context: ErrorContext,
}

impl<E: AppError> DistributedError<E> {
    /// Wraps `error` together with `context`.
    pub fn new(error: E, context: ErrorContext) -> Self {
        Self { error, context }
    }

    /// Emits a single structured `tracing` event describing the error.
    ///
    /// The event's level is derived from the error's
    /// [`Severity::log_level`](crate::Severity::log_level), and it carries every
    /// context field plus the error's code/category/severity so log pipelines
    /// can index and alert on them. The internal trace/span ids stay here (in
    /// the logs) and are never sent to clients.
    pub fn log(&self) {
        let ctx = &self.context;
        let severity = self.error.severity();

        // One macro, expanded per level: `tracing`'s typed macros require a
        // statically-known level, so we dispatch on the dynamic level once.
        macro_rules! emit {
            ($lvl:ident) => {
                tracing::$lvl!(
                    request_id = %ctx.request_id,
                    trace_id = ?ctx.trace_id,
                    span_id = ?ctx.span_id,
                    service = ctx.service_name,
                    severity = %severity,
                    error_code = self.error.error_code(),
                    category = self.error.category(),
                    retryable = self.error.is_retryable(),
                    error.message = %self.error,
                    "distributed error captured"
                )
            };
        }

        let level = severity.log_level();
        if level == Level::ERROR {
            emit!(error);
        } else if level == Level::WARN {
            emit!(warn);
        } else if level == Level::INFO {
            emit!(info);
        } else if level == Level::DEBUG {
            emit!(debug);
        } else {
            emit!(trace);
        }
    }
}

impl<E: AppError> fmt::Display for DistributedError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{service}] {code}: {error} (request_id={request_id})",
            service = self.context.service_name,
            code = self.error.error_code(),
            error = self.error,
            request_id = self.context.request_id,
        )
    }
}

impl<E: AppError> std::error::Error for DistributedError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}
