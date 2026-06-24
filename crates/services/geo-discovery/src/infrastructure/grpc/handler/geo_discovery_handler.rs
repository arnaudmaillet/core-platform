use tonic::{Request, Response, Status};
use uuid::Uuid;

use cqrs::{Envelope, QueryBus};

use crate::application::query::query_tile::QueryTileQuery;

// ── Proto inclusion ───────────────────────────────────────────────────────────

pub use geo_discovery_api as proto;

pub use proto::geo_discovery_service_server::GeoDiscoveryServiceServer;

// ── Handler ───────────────────────────────────────────────────────────────────

pub struct GeoDiscoveryHandler<QB>
where
    QB: QueryBus + Send + Sync + 'static,
{
    query_bus: QB,
}

impl<QB> GeoDiscoveryHandler<QB>
where
    QB: QueryBus + Send + Sync + 'static,
{
    pub fn new(query_bus: QB) -> Self {
        Self { query_bus }
    }
}

// ── RPC implementations ───────────────────────────────────────────────────────

impl<QB> GeoDiscoveryHandler<QB>
where
    QB: QueryBus + Send + Sync + 'static,
{
    async fn query_tile_inner(
        &self,
        request: Request<proto::QueryTileRequest>,
    ) -> Result<Response<proto::QueryTileResponse>, Status> {
        let req      = request.into_inner();
        let viewport = req.viewport.ok_or_else(|| Status::invalid_argument("viewport is required"))?;

        let query = QueryTileQuery {
            sw_lat:     viewport.sw_lat,
            sw_lng:     viewport.sw_lng,
            ne_lat:     viewport.ne_lat,
            ne_lng:     viewport.ne_lng,
            zoom_level: req.zoom_level,
        };

        let result = self.query_bus
            .dispatch(Envelope::new(Uuid::now_v7(), query))
            .await
            .map_err(cqrs_to_status)?;

        let cards = result.cards
            .into_iter()
            .map(card_to_proto)
            .collect();

        Ok(Response::new(proto::QueryTileResponse {
            cards,
            tile_count: result.tile_count,
        }))
    }

    async fn get_card_inner(
        &self,
        _request: Request<proto::GetCardRequest>,
    ) -> Result<Response<proto::GetCardResponse>, Status> {
        // GetCard is handled via QueryBus like QueryTile in a full implementation.
        // Stubbed here to keep the example concise; add a GetCardQuery following
        // the same pattern as QueryTileQuery.
        Err(Status::unimplemented("GetCard not yet implemented"))
    }
}

// ── Proto trait implementation ─────────────────────────────────────────────────

#[tonic::async_trait]
impl<QB> proto::geo_discovery_service_server::GeoDiscoveryService for GeoDiscoveryHandler<QB>
where
    QB: QueryBus + Send + Sync + 'static,
{
    async fn query_tile(
        &self,
        request: Request<proto::QueryTileRequest>,
    ) -> Result<Response<proto::QueryTileResponse>, Status> {
        self.query_tile_inner(request).await
    }

    async fn get_card(
        &self,
        request: Request<proto::GetCardRequest>,
    ) -> Result<Response<proto::GetCardResponse>, Status> {
        self.get_card_inner(request).await
    }
}

// ── Conversion helpers ────────────────────────────────────────────────────────

fn card_to_proto(card: crate::domain::entity::MapPostCard) -> proto::MapPostCard {
    // Map u8 tier (0=Standard, 1=Premium, 2=VIP) to proto AuthorTier enum.
    // Proto uses +1 offset: UNSPECIFIED=0, STANDARD=1, PREMIUM=2, VIP=3.
    // We treat u8=0 as STANDARD (not UNSPECIFIED) for deterministic client rendering.
    let author_tier = match card.author_tier {
        1 => proto::AuthorTier::Premium as i32,
        2 => proto::AuthorTier::Vip as i32,
        _ => proto::AuthorTier::Standard as i32,
    };

    proto::MapPostCard {
        post_id:           card.post_id.to_string(),
        author_id:         card.author_id.to_string(),
        author_handle:     card.author_handle,
        author_avatar_url: card.author_avatar_url,
        thumbnail_url:     card.thumbnail_url,
        h3_index_r7:       card.h3_index_r7,
        virality_score:    card.virality_score,
        published_at_ms:   card.published_at_ms,
        author_tier,
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
