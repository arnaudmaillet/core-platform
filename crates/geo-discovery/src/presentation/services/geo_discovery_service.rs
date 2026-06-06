// crates/geo_discovery/src/presentation/grpc/services.rs

use crate::context::GeoDiscoveryQueryContext;
use crate::types::{MapViewport, TileResolution};
use shared_proto::geo_discovery::v1::{
    GetMapPinsRequest, GetMapPinsResponse,
    geo_discovery_service_server::GeoDiscoveryService as ProtoGeoDiscoveryService,
};
use tonic::{Request, Response, Status};

pub struct GeoDiscoveryService {
    query_ctx: GeoDiscoveryQueryContext,
}

impl GeoDiscoveryService {
    pub fn new(query_ctx: GeoDiscoveryQueryContext) -> Self {
        Self { query_ctx }
    }
}

#[tonic::async_trait]
impl ProtoGeoDiscoveryService for GeoDiscoveryService {
    async fn get_map_pins(
        &self,
        request: Request<GetMapPinsRequest>,
    ) -> Result<Response<GetMapPinsResponse>, Status> {
        let req = request.into_inner();

        let protobuf_viewport = req
            .viewport
            .ok_or_else(|| Status::invalid_argument("Missing required 'viewport' bounding box"))?;

        let south_west = protobuf_viewport.south_west.ok_or_else(|| {
            Status::invalid_argument("Missing 'south_west' coordinates in viewport")
        })?;

        let north_east = protobuf_viewport.north_east.ok_or_else(|| {
            Status::invalid_argument("Missing 'north_east' coordinates in viewport")
        })?;

        let viewport = MapViewport::try_new(south_west, north_east)
            .map_err(|e| Status::invalid_argument(format!("Invalid viewport geometry: {}", e)))?;

        let resolution = TileResolution::from_client_zoom_int(req.zoom_level);

        let limit = if req.limit_per_tile == 0 {
            50 // Valeur par défaut protectrice pour l'infra
        } else {
            req.limit_per_tile as usize
        };

        let pins = self
            .query_ctx
            .get_map_pins(viewport, resolution, limit)
            .await
            .map_err(|e| Status::internal(format!("Failed to resolve map feed: {}", e)))?;

        Ok(Response::new(GetMapPinsResponse { pins }))
    }
}
