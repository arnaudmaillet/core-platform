use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};
use validate_core::{FieldViolation, Validate};

use crate::application::port::{CardStore, SpatialIndex, TileRepository};
use crate::domain::entity::MapPostCard;
use crate::domain::value_object::{GeoCoordinate, zoom_to_resolution};
use crate::error::GeoDiscoveryError;
use crate::infrastructure::h3::h3_codec;

/// Queries the spatial index for all visible post cards within a viewport.
///
/// Hot path: 2 Redis round-trips (ZRANGEBYSCORE × N tiles → MGET × M cards).
/// Fallback: ScyllaDB point-reads for cards not present in Redis (cache miss).
pub struct QueryTileQuery {
    pub sw_lat:     f64,
    pub sw_lng:     f64,
    pub ne_lat:     f64,
    pub ne_lng:     f64,
    pub zoom_level: i32,
}

pub struct QueryTileResult {
    pub cards:      Vec<MapPostCard>,
    pub tile_count: i32,
}

impl Query for QueryTileQuery {
    type Response = QueryTileResult;
}

impl Validate for QueryTileQuery {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if !self.sw_lat.is_finite() || self.sw_lat < -90.0 || self.sw_lat > 90.0 {
            v.push(FieldViolation::new("sw_lat", "GEO-VAL-030", "sw_lat must be in [-90, 90]"));
        }
        if !self.sw_lng.is_finite() || self.sw_lng < -180.0 || self.sw_lng > 180.0 {
            v.push(FieldViolation::new("sw_lng", "GEO-VAL-031", "sw_lng must be in [-180, 180]"));
        }
        if !self.ne_lat.is_finite() || self.ne_lat < -90.0 || self.ne_lat > 90.0 {
            v.push(FieldViolation::new("ne_lat", "GEO-VAL-032", "ne_lat must be in [-90, 90]"));
        }
        if !self.ne_lng.is_finite() || self.ne_lng < -180.0 || self.ne_lng > 180.0 {
            v.push(FieldViolation::new("ne_lng", "GEO-VAL-033", "ne_lng must be in [-180, 180]"));
        }
        if self.sw_lat >= self.ne_lat {
            v.push(FieldViolation::new("viewport", "GEO-VAL-034", "sw_lat must be less than ne_lat"));
        }
        if self.zoom_level < 0 || self.zoom_level > 15 {
            v.push(FieldViolation::new("zoom_level", "GEO-VAL-035", "zoom_level must be in [0, 15]"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct QueryTileHandler<SI, CS, TR> {
    pub spatial_index:   Arc<SI>,
    pub card_store:      Arc<CS>,
    pub tile_repository: Arc<TR>,
}

impl<SI, CS, TR> QueryHandler<QueryTileQuery> for QueryTileHandler<SI, CS, TR>
where
    SI: SpatialIndex + 'static,
    CS: CardStore + 'static,
    TR: TileRepository + 'static,
{
    type Error = GeoDiscoveryError;

    async fn handle(&self, envelope: Envelope<QueryTileQuery>) -> Result<QueryTileResult, GeoDiscoveryError> {
        let q = &envelope.payload;

        let sw = GeoCoordinate::new(q.sw_lat, q.sw_lng)?;
        let ne = GeoCoordinate::new(q.ne_lat, q.ne_lng)?;

        if sw.lat >= ne.lat {
            return Err(GeoDiscoveryError::InvalidViewport {
                sw_lat: q.sw_lat, sw_lng: q.sw_lng,
                ne_lat: q.ne_lat, ne_lng: q.ne_lng,
            });
        }

        let resolution  = zoom_to_resolution(q.zoom_level);
        let min_score   = resolution.virality_floor(q.zoom_level);
        let tiles       = h3_codec::viewport_cells(&sw, &ne, resolution);
        let tile_count  = tiles.len() as i32;

        // ── Phase 1: ZRANGEBYSCORE for all tiles (concurrent, one RTT per tile
        //   through fred's lock-free command queue → effectively pipelined) ────
        let tile_futures: Vec<_> = tiles.iter()
            .map(|tile| {
                let tile   = *tile;
                let si     = Arc::clone(&self.spatial_index);
                async move { si.query(tile, resolution, min_score).await }
            })
            .collect();

        let tile_results = futures::future::try_join_all(tile_futures).await?;

        // Deduplicate across tile boundaries (posts near hexagon edges appear in
        // multiple grid_disk results).
        let mut seen = std::collections::HashSet::new();
        let post_ids: Vec<uuid::Uuid> = tile_results
            .into_iter()
            .flatten()
            .filter(|id| seen.insert(*id))
            .collect();

        if post_ids.is_empty() {
            // Touch hot tiles even for empty results (keeps active-area tiles warm).
            let touch_pairs: Vec<_> = tiles.iter().map(|t| (*t, resolution)).collect();
            let _ = self.spatial_index.touch_hot_tiles(&touch_pairs).await;
            return Ok(QueryTileResult { cards: vec![], tile_count });
        }

        // ── Phase 2: MGET for all cards (single round-trip) ──────────────────
        let cached = self.card_store.mget(&post_ids).await?;

        // ── Phase 3: ScyllaDB fallback for cache misses ───────────────────────
        let mut cards: Vec<MapPostCard> = Vec::with_capacity(post_ids.len());
        let mut miss_ids: Vec<uuid::Uuid> = Vec::new();

        for (id, opt_card) in post_ids.iter().zip(cached) {
            match opt_card {
                Some(card) => cards.push(card),
                None       => miss_ids.push(*id),
            }
        }

        if !miss_ids.is_empty() {
            let miss_futures: Vec<_> = miss_ids.iter()
                .map(|id| {
                    let post_id = crate::domain::value_object::PostId::from(*id);
                    let tr = Arc::clone(&self.tile_repository);
                    async move { tr.get_card(&post_id).await }
                })
                .collect();

            let miss_results = futures::future::try_join_all(miss_futures).await?;
            for maybe_card in miss_results.into_iter().flatten() {
                cards.push(maybe_card);
            }
        }

        // Fire-and-forget: update hot_tiles scores for the queried tiles.
        let touch_pairs: Vec<_> = tiles.iter().map(|t| (*t, resolution)).collect();
        let _ = self.spatial_index.touch_hot_tiles(&touch_pairs).await;

        Ok(QueryTileResult { cards, tile_count })
    }
}
