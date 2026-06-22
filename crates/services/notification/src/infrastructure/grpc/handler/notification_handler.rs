use std::pin::Pin;
use std::sync::Arc;

use futures::Stream;
use tonic::{Request, Response, Status};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt as _;
use uuid::Uuid;

use cqrs::{CommandBus, Envelope, QueryBus};

use crate::application::command::mark_read::{MarkAllReadCommand, MarkReadCommand};
use crate::application::port::{NotificationSummary, StreamRegistry};
use crate::application::query::{
    get_unread_count::GetUnreadCountQuery,
    list_notifications::ListNotificationsQuery,
};
use crate::domain::value_object::ProfileId;

// ── Proto inclusion ───────────────────────────────────────────────────────────

pub mod proto {
    tonic::include_proto!("notification.v1");
}

pub use proto::notification_service_server::NotificationServiceServer;

// ── Handler struct ────────────────────────────────────────────────────────────

pub struct NotificationServiceHandler<CB, QB, SR>
where
    CB: CommandBus + Send + Sync + 'static,
    QB: QueryBus + Send + Sync + 'static,
    SR: StreamRegistry + Send + Sync + 'static,
{
    command_bus:     CB,
    query_bus:       QB,
    stream_registry: Arc<SR>,
}

impl<CB, QB, SR> NotificationServiceHandler<CB, QB, SR>
where
    CB: CommandBus + Send + Sync + 'static,
    QB: QueryBus + Send + Sync + 'static,
    SR: StreamRegistry + Send + Sync + 'static,
{
    pub fn new(command_bus: CB, query_bus: QB, stream_registry: Arc<SR>) -> Self {
        Self { command_bus, query_bus, stream_registry }
    }
}

// ── RPC implementations ───────────────────────────────────────────────────────

impl<CB, QB, SR> NotificationServiceHandler<CB, QB, SR>
where
    CB: CommandBus + Send + Sync + 'static,
    QB: QueryBus + Send + Sync + 'static,
    SR: StreamRegistry + Send + Sync + 'static,
{
    pub async fn list_notifications(
        &self,
        request: Request<proto::ListNotificationsRequest>,
    ) -> Result<Response<proto::ListNotificationsResponse>, Status> {
        let req = request.into_inner();
        let query = ListNotificationsQuery {
            profile_id: req.profile_id,
            limit:      req.limit,
            page_token: Some(req.page_token).filter(|s| !s.is_empty()),
        };

        let page = self
            .query_bus
            .dispatch(Envelope::new(Uuid::now_v7(), query))
            .await
            .map_err(cqrs_to_status)?;

        Ok(Response::new(proto::ListNotificationsResponse {
            notifications:   page.notifications.into_iter().map(summary_to_proto).collect(),
            next_page_token: page.next_page_token.unwrap_or_default(),
            read_horizon_ms: page.read_horizon_ms,
        }))
    }

    pub async fn get_unread_count(
        &self,
        request: Request<proto::GetUnreadCountRequest>,
    ) -> Result<Response<proto::GetUnreadCountResponse>, Status> {
        let query = GetUnreadCountQuery { profile_id: request.into_inner().profile_id };

        let count: i64 = self
            .query_bus
            .dispatch(Envelope::new(Uuid::now_v7(), query))
            .await
            .map_err(cqrs_to_status)?;

        Ok(Response::new(proto::GetUnreadCountResponse { unread_count: count }))
    }

    pub async fn mark_read(
        &self,
        request: Request<proto::MarkReadRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        let req = request.into_inner();
        let cmd = MarkReadCommand {
            profile_id:      req.profile_id,
            notification_id: req.notification_id,
            created_at_ms:   req.created_at_ms,
        };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .map(|_| ok_response())
            .map_err(cqrs_to_status)
    }

    pub async fn mark_all_read(
        &self,
        request: Request<proto::MarkAllReadRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        let cmd = MarkAllReadCommand { profile_id: request.into_inner().profile_id };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .map(|_| ok_response())
            .map_err(cqrs_to_status)
    }

    pub async fn stream_notifications(
        &self,
        request: Request<proto::StreamNotificationsRequest>,
    ) -> Result<
        Response<Pin<Box<dyn Stream<Item = Result<proto::StreamNotificationsResponse, Status>> + Send + 'static>>>,
        Status,
    > {
        let profile_id = ProfileId::try_from(request.into_inner().profile_id.as_str())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let rx     = self.stream_registry.subscribe(&profile_id);
        let stream = BroadcastStream::new(rx).filter_map(|result| {
            match result {
                Ok(payload) => {
                    let view = payload_to_proto(&payload);
                    Some(Ok(proto::StreamNotificationsResponse {
                        notification: Some(view),
                    }))
                }
                Err(_lagged) => {
                    // Receiver fell behind — the client must re-poll ListNotifications.
                    // Terminating the stream signals the client to reconnect.
                    Some(Err(Status::data_loss(
                        "stream lagged: re-poll ListNotifications to recover missed notifications"
                    )))
                }
            }
        });

        Ok(Response::new(Box::pin(stream)))
    }
}

// ── Proto trait implementation ────────────────────────────────────────────────

#[tonic::async_trait]
impl<CB, QB, SR> proto::notification_service_server::NotificationService
    for NotificationServiceHandler<CB, QB, SR>
where
    CB: CommandBus + Send + Sync + 'static,
    QB: QueryBus + Send + Sync + 'static,
    SR: StreamRegistry + Send + Sync + 'static,
{
    type StreamNotificationsStream =
        Pin<Box<dyn Stream<Item = Result<proto::StreamNotificationsResponse, Status>> + Send + 'static>>;

    async fn list_notifications(
        &self,
        request: Request<proto::ListNotificationsRequest>,
    ) -> Result<Response<proto::ListNotificationsResponse>, Status> {
        self.list_notifications(request).await
    }

    async fn get_unread_count(
        &self,
        request: Request<proto::GetUnreadCountRequest>,
    ) -> Result<Response<proto::GetUnreadCountResponse>, Status> {
        self.get_unread_count(request).await
    }

    async fn mark_read(
        &self,
        request: Request<proto::MarkReadRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        self.mark_read(request).await
    }

    async fn mark_all_read(
        &self,
        request: Request<proto::MarkAllReadRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        self.mark_all_read(request).await
    }

    async fn stream_notifications(
        &self,
        request: Request<proto::StreamNotificationsRequest>,
    ) -> Result<Response<Self::StreamNotificationsStream>, Status> {
        self.stream_notifications(request).await
    }
}

// ── Conversion helpers ────────────────────────────────────────────────────────

fn ok_response() -> Response<proto::CommandResponse> {
    Response::new(proto::CommandResponse { success: true, message: String::new() })
}

fn summary_to_proto(s: NotificationSummary) -> proto::NotificationView {
    proto::NotificationView {
        notification_id:   s.notification_id.to_string(),
        target_profile_id: s.target_profile_id.to_string(),
        sender_profile_id: s.sender_profile_id.to_string(),
        sample_sender_ids: s.sample_sender_ids.iter().map(|u| u.to_string()).collect(),
        sender_count:      s.sender_count,
        kind:              s.kind.as_tinyint() as i32,
        subject_kind:      s.subject_kind.as_tinyint() as i32,
        subject_id:        s.subject_id.to_string(),
        created_at_ms:     s.created_at.timestamp_millis(),
        is_read:           s.is_read,
    }
}

fn payload_to_proto(
    p: &crate::application::port::stream_registry::NotificationPayload,
) -> proto::NotificationView {
    proto::NotificationView {
        notification_id:   p.notification_id.to_string(),
        target_profile_id: p.target_profile_id.to_string(),
        sender_profile_id: p.sender_profile_id.to_string(),
        sample_sender_ids: p.sample_sender_ids.iter().map(|u| u.to_string()).collect(),
        sender_count:      p.sender_count,
        kind:              p.kind.as_tinyint() as i32,
        subject_kind:      p.subject_kind.as_tinyint() as i32,
        subject_id:        p.subject_id.to_string(),
        created_at_ms:     p.created_at_ms,
        is_read:           false,
    }
}

// ── Error mapping ─────────────────────────────────────────────────────────────

pub fn cqrs_to_status(err: cqrs::error::CqrsError) -> Status {
    use cqrs::error::CqrsError;
    match err {
        CqrsError::HandlerNotFound { type_name } => {
            Status::unimplemented(format!("no handler registered for {type_name}"))
        }
        CqrsError::DuplicateRegistration { type_name } => {
            Status::internal(format!("duplicate handler for {type_name}"))
        }
        CqrsError::Handler(boxed) => {
            use error::AppError as _;
            let msg       = boxed.to_string();
            let retryable = boxed.is_retryable();
            match boxed.http_status().as_u16() {
                403       => Status::permission_denied(msg),
                404       => Status::not_found(msg),
                409 if retryable => Status::aborted(msg),
                409       => Status::already_exists(msg),
                400 | 422 => Status::failed_precondition(msg),
                503 | 502 => Status::unavailable(msg),
                _         => Status::internal(msg),
            }
        }
    }
}
