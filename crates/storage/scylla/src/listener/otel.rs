use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use dashmap::DashMap;
use scylla::errors::{RequestAttemptError, RequestError};
use scylla::observability::history::{AttemptId, HistoryListener, RequestId, SpeculativeId};
use scylla::policies::retry::RetryDecision;
use tracing::Span;

/// Bridges ScyllaDB per-statement execution events into the process-global
/// `tracing` subscriber installed by the `telemetry` crate.
///
/// In scylla 1.x the `HistoryListener` is **per-statement**, not per-session.
/// Attach a shared `Arc<OtelHistoryListener>` to each statement before
/// execution:
///
/// ```rust,ignore
/// use std::sync::Arc;
/// use scylla::observability::history::HistoryListener;
/// use scylla::statement::unprepared::Statement;
///
/// let mut stmt = Statement::new("INSERT INTO feed.events ...");
/// stmt.set_history_listener(Arc::clone(&client.history_listener) as Arc<dyn HistoryListener>);
/// session.query_unpaged(stmt, values).await?;
/// ```
///
/// The same `Arc` can be shared across every statement in the service —
/// internal state is protected by lock-free `DashMap` shards.
///
/// ## Span hierarchy
///
/// ```text
/// [caller's active span]
///   └── scylla.request               ← full query lifecycle
///         ├── scylla.attempt         ← primary coordinator round-trip
///         └── scylla.attempt         ← speculative backup (if fired)
/// ```
///
/// All spans inherit the active OTel trace context from the calling task,
/// so they appear as children inside the correct distributed trace without
/// any manual wiring.
pub struct OtelHistoryListener {
    next_request_id:    AtomicUsize,
    next_speculative_id: AtomicUsize,
    next_attempt_id:    AtomicUsize,
    request_spans:      DashMap<usize, Span>,
    speculative_spans:  DashMap<usize, Span>,
    attempt_spans:      DashMap<usize, Span>,
}

impl fmt::Debug for OtelHistoryListener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OtelHistoryListener")
            .field("active_requests",   &self.request_spans.len())
            .field("active_speculative", &self.speculative_spans.len())
            .field("active_attempts",   &self.attempt_spans.len())
            .finish()
    }
}

impl OtelHistoryListener {
    pub fn new() -> Self {
        Self {
            next_request_id:     AtomicUsize::new(0),
            next_speculative_id: AtomicUsize::new(0),
            next_attempt_id:     AtomicUsize::new(0),
            request_spans:       DashMap::new(),
            speculative_spans:   DashMap::new(),
            attempt_spans:       DashMap::new(),
        }
    }

    /// Convenience constructor that wraps `Self` in an `Arc`, ready to be
    /// cloned into individual statement configurations.
    pub fn arc() -> Arc<Self> {
        Arc::new(Self::new())
    }
}

impl Default for OtelHistoryListener {
    fn default() -> Self {
        Self::new()
    }
}

impl HistoryListener for OtelHistoryListener {
    /// Called by the driver when a query is dispatched — before any coordinator
    /// is contacted. Creates the root `scylla.request` span as a child of the
    /// currently-active `tracing` span (the calling task's OTel context).
    fn log_request_start(&self) -> RequestId {
        let id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let span = tracing::info_span!(
            "scylla.request",
            "otel.kind"  = "CLIENT",
            "db.system"  = "scylladb",
            error        = tracing::field::Empty,
            "error.message" = tracing::field::Empty,
        );
        self.request_spans.insert(id, span);
        RequestId(id)
    }

    /// Called when a query completes successfully. Closes the root span.
    fn log_request_success(&self, request_id: RequestId) {
        if let Some((_, span)) = self.request_spans.remove(&request_id.0) {
            let _guard = span.enter();
            tracing::debug!("scylla request succeeded");
        }
    }

    /// Called when a query fails after all retry/speculative attempts are
    /// exhausted. Records the terminal error and closes the root span.
    fn log_request_error(&self, request_id: RequestId, error: &RequestError) {
        if let Some((_, span)) = self.request_spans.remove(&request_id.0) {
            span.record("error", true);
            span.record("error.message", error.to_string().as_str());
            let _guard = span.enter();
            tracing::warn!(error = %error, "scylla request failed");
        }
    }

    /// Called when the driver decides to fire a speculative backup request.
    /// Creates a child span of the parent `scylla.request` span.
    fn log_new_speculative_fiber(&self, request_id: RequestId) -> SpeculativeId {
        let id = self.next_speculative_id.fetch_add(1, Ordering::Relaxed);
        let parent: Option<Span> = self
            .request_spans
            .get(&request_id.0)
            .map(|entry| entry.value().clone());

        let speculative_span = match &parent {
            Some(p) => p.in_scope(|| {
                tracing::info_span!(
                    "scylla.speculative_fiber",
                    "otel.kind" = "CLIENT",
                    "db.system" = "scylladb",
                    speculative = true,
                )
            }),
            None => tracing::info_span!(
                "scylla.speculative_fiber",
                "otel.kind" = "CLIENT",
                "db.system" = "scylladb",
                speculative = true,
            ),
        };

        self.speculative_spans.insert(id, speculative_span);
        SpeculativeId(id)
    }

    /// Called when the driver sends a request to a specific coordinator node.
    /// Creates a child span of either the speculative-fiber span (if this is a
    /// speculative attempt) or the root request span (primary attempt).
    fn log_attempt_start(
        &self,
        request_id:    RequestId,
        speculative_id: Option<SpeculativeId>,
        node_addr:     SocketAddr,
    ) -> AttemptId {
        let id = self.next_attempt_id.fetch_add(1, Ordering::Relaxed);

        let parent: Option<Span> = match speculative_id {
            Some(sid) => self
                .speculative_spans
                .get(&sid.0)
                .map(|entry| entry.value().clone()),
            None => self
                .request_spans
                .get(&request_id.0)
                .map(|entry| entry.value().clone()),
        };

        let attempt_span = match &parent {
            Some(p) => p.in_scope(|| {
                tracing::info_span!(
                    "scylla.attempt",
                    "otel.kind"     = "CLIENT",
                    "db.system"     = "scylladb",
                    "net.peer.name" = %node_addr.ip(),
                    "net.peer.port" = node_addr.port(),
                    retry_decision  = tracing::field::Empty,
                    error           = tracing::field::Empty,
                    "error.message" = tracing::field::Empty,
                )
            }),
            None => tracing::info_span!(
                "scylla.attempt",
                "otel.kind"     = "CLIENT",
                "db.system"     = "scylladb",
                "net.peer.name" = %node_addr.ip(),
                "net.peer.port" = node_addr.port(),
                retry_decision  = tracing::field::Empty,
                error           = tracing::field::Empty,
                "error.message" = tracing::field::Empty,
            ),
        };

        self.attempt_spans.insert(id, attempt_span);
        AttemptId(id)
    }

    /// Called when the coordinator returned a successful response. Closes the
    /// attempt span at `DEBUG` level (success is the hot path, not noteworthy).
    fn log_attempt_success(&self, attempt_id: AttemptId) {
        if let Some((_, span)) = self.attempt_spans.remove(&attempt_id.0) {
            let _guard = span.enter();
            tracing::debug!("scylla attempt succeeded");
        }
    }

    /// Called when the coordinator returned an error. Records the error
    /// message and driver retry decision, then closes the attempt span.
    fn log_attempt_error(
        &self,
        attempt_id:     AttemptId,
        error:          &RequestAttemptError,
        retry_decision: &RetryDecision,
    ) {
        if let Some((_, span)) = self.attempt_spans.remove(&attempt_id.0) {
            span.record("error", true);
            span.record("error.message", error.to_string().as_str());
            span.record("retry_decision", format!("{retry_decision:?}").as_str());
            let _guard = span.enter();
            tracing::warn!(
                error          = %error,
                retry_decision = ?retry_decision,
                "scylla attempt failed",
            );
        }
    }
}
