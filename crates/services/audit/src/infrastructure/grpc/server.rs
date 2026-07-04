use tonic::{Request, Response, Status};

use super::handler::{AuditServiceHandler, proto};
use proto::audit_service_server::AuditService;

/// Encoded protobuf descriptor set for gRPC server reflection, emitted by
/// `audit-api`'s `build.rs`.
pub const FILE_DESCRIPTOR_SET: &[u8] = audit_api::FILE_DESCRIPTOR_SET;

#[tonic::async_trait]
impl AuditService for AuditServiceHandler {
    async fn record_privileged(
        &self,
        request: Request<proto::RecordPrivilegedRequest>,
    ) -> Result<Response<proto::RecordPrivilegedResponse>, Status> {
        self.record_privileged(request).await
    }

    async fn query(
        &self,
        request: Request<proto::QueryRequest>,
    ) -> Result<Response<proto::QueryResponse>, Status> {
        self.query(request).await
    }

    async fn export(
        &self,
        request: Request<proto::ExportRequest>,
    ) -> Result<Response<proto::ExportManifest>, Status> {
        self.export(request).await
    }

    async fn verify_integrity(
        &self,
        request: Request<proto::VerifyIntegrityRequest>,
    ) -> Result<Response<proto::VerifyIntegrityResponse>, Status> {
        self.verify_integrity(request).await
    }
}
