//! gRPC request handler for `audit.v1`. Each method maps an inbound Protobuf
//! request into domain values (via [`super::super::codec`]), runs the matching
//! application use case, and maps the result — or [`AuditError`] — back to
//! Protobuf / [`Status`].
//!
//! The synchronous `RecordPrivileged` lane is **fail-closed**: it is wrapped in a
//! hard durable-commit deadline (`record_timeout`); on elapse it returns
//! `AUD-4004` so the caller denies the privileged action rather than performing it
//! unrecorded.
//!
//! Authorization (need-to-know + separation of duties → `AUD-3001`/`AUD-3002`) and
//! the recording of each read as its own `DATA_ACCESS` event are a deployment
//! concern wired via the `auth-context` ingress interceptor (a documented
//! requirement, like the realtime edge-token verification) — the handler is the
//! authorized read behind that gate.

use std::sync::Arc;
use std::time::Duration;

use error::AppError;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::application::dto::LedgerQuery;
use crate::application::{ExportHandler, QueryHandler, RecordPrivilegedHandler, VerifyHandler};
use crate::domain::PartitionKey;
use crate::error::AuditError;
use crate::infrastructure::codec;

pub use audit_api as proto;

/// Hard cap on records returned by a single query/export page.
const MAX_PAGE: usize = 500;

/// gRPC handler for `audit.v1.AuditService`. Holds the four use-case handlers.
#[derive(Clone)]
pub struct AuditServiceHandler {
    record: Arc<RecordPrivilegedHandler>,
    query: Arc<QueryHandler>,
    export: Arc<ExportHandler>,
    verify: Arc<VerifyHandler>,
    record_timeout: Duration,
}

impl AuditServiceHandler {
    pub fn new(
        record: Arc<RecordPrivilegedHandler>,
        query: Arc<QueryHandler>,
        export: Arc<ExportHandler>,
        verify: Arc<VerifyHandler>,
        record_timeout: Duration,
    ) -> Self {
        Self {
            record,
            query,
            export,
            verify,
            record_timeout,
        }
    }

    pub async fn record_privileged(
        &self,
        request: Request<proto::RecordPrivilegedRequest>,
    ) -> Result<Response<proto::RecordPrivilegedResponse>, Status> {
        let req = request.into_inner();
        let event = codec::event_from_pb(req.event.ok_or_else(|| {
            Status::invalid_argument("record_privileged requires an event")
        })?)
        .map_err(to_status)?;
        let action = codec::privileged_action_from_pb(req.privileged_action).map_err(to_status)?;

        // Fail-closed: a slow store elapses into AUD-4004, denying the action
        // rather than letting it proceed unrecorded.
        let proof = match tokio::time::timeout(self.record_timeout, self.record.record(event, action))
            .await
        {
            Ok(result) => result.map_err(to_status)?,
            Err(_elapsed) => return Err(to_status(AuditError::DurabilityNotConfirmed)),
        };

        Ok(Response::new(codec::proof_to_pb(&proof)))
    }

    pub async fn query(
        &self,
        request: Request<proto::QueryRequest>,
    ) -> Result<Response<proto::QueryResponse>, Status> {
        let spec = codec::query_from_pb(request.into_inner()).map_err(to_status)?;
        let records = self.query.query(&spec).await.map_err(to_status)?;
        Ok(Response::new(proto::QueryResponse {
            records: records.iter().map(codec::record_to_pb).collect(),
            // Pagination beyond a single capped page is deferred (Phase 7).
            next_page_token: String::new(),
        }))
    }

    pub async fn export(
        &self,
        request: Request<proto::ExportRequest>,
    ) -> Result<Response<proto::ExportManifest>, Status> {
        let req = request.into_inner();
        let spec = LedgerQuery {
            subject: opt(req.subject_pseudonym)
                .map(crate::domain::SubjectPseudonym::new)
                .transpose()
                .map_err(to_status)?,
            tenant: opt(req.tenant_id)
                .map(crate::domain::TenantId::new)
                .transpose()
                .map_err(to_status)?,
            category: None,
            from: req.from.as_ref().map(ts_from_pb),
            to: req.to.as_ref().map(ts_from_pb),
            limit: MAX_PAGE,
        };
        let export_id = Uuid::now_v7().to_string();
        let manifest = self.export.export(&export_id, &spec).await.map_err(to_status)?;
        Ok(Response::new(codec::export_manifest_to_pb(&manifest)))
    }

    pub async fn verify_integrity(
        &self,
        request: Request<proto::VerifyIntegrityRequest>,
    ) -> Result<Response<proto::VerifyIntegrityResponse>, Status> {
        let req = request.into_inner();
        let report = if req.partition_key.trim().is_empty() {
            // Empty partition → the global head-vs-anchor check.
            self.verify.verify_global().await.map_err(to_status)?
        } else {
            let partition = PartitionKey::new(req.partition_key).map_err(to_status)?;
            self.verify.verify_partition(&partition).await.map_err(to_status)?
        };
        Ok(Response::new(codec::integrity_report_to_pb(&report)))
    }
}

fn opt(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn ts_from_pb(ts: &prost_types::Timestamp) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(ts.seconds, ts.nanos.max(0) as u32)
        .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap())
}

/// Map a domain fault to a gRPC `Status` by its canonical HTTP status. PII and
/// payloads are never in these messages (only stable AUD-XXXX-coded text).
fn to_status(err: AuditError) -> Status {
    let message = err.to_string();
    match err.http_status().as_u16() {
        400 | 422 => Status::invalid_argument(message),
        403 => Status::permission_denied(message),
        404 | 410 => Status::not_found(message),
        409 => Status::aborted(message),
        503 => Status::unavailable(message),
        504 => Status::deadline_exceeded(message),
        _ => Status::internal(message),
    }
}
