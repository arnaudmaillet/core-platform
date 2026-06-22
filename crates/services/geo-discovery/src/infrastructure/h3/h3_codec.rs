use crate::domain::value_object::{GeoCoordinate, H3Index, H3Resolution};

/// Converts a viewport bounding box into the set of H3 cells that cover it
/// at the given resolution.
///
/// Strategy: grid_disk around the viewport center with a k-ring radius computed
/// from the viewport's diagonal and the resolution's hexagon edge length.
/// This avoids the `geo` crate dependency while providing good coverage for
/// typical rectangular map viewports. Cells outside the bbox are included in
/// the k-ring; the resulting over-fetch is small (≤ 1 ring width) and harmless
/// — ZRANGEBYSCORE returns empty for tiles with no posts.
///
/// Radius is clamped to 50 to prevent pathologically wide viewports (zoom 1)
/// from issuing thousands of tile queries.
pub fn viewport_cells(
    sw:         &GeoCoordinate,
    ne:         &GeoCoordinate,
    resolution: H3Resolution,
) -> Vec<H3Index> {
    let center_lat = (sw.lat + ne.lat) / 2.0;
    let center_lng = (sw.lng + ne.lng) / 2.0;

    let center = GeoCoordinate::new(center_lat, center_lng)
        .expect("viewport center derived from validated corners is always valid");

    let center_cell = H3Index::encode(&center, resolution);

    let k = compute_k_ring(sw, ne, &center, resolution);

    center_cell.grid_disk(k)
}

/// Computes the k-ring radius needed to cover the viewport.
///
/// Uses the viewport's maximum span (lat or lng, converted to km) divided by
/// the hexagon diameter at the target resolution. Adds +2 for border padding
/// and clamps to [1, 50].
fn compute_k_ring(
    sw:         &GeoCoordinate,
    ne:         &GeoCoordinate,
    center:     &GeoCoordinate,
    resolution: H3Resolution,
) -> u32 {
    let lat_span_km = (ne.lat - sw.lat).abs() * 111.0;
    let lng_span_km = (ne.lng - sw.lng).abs()
        * 111.0
        * center.lat.to_radians().cos().abs().max(0.01);

    let max_span_km = lat_span_km.max(lng_span_km);

    // Hexagon diameter ≈ 2 × edge_length (approximate).
    let hex_diameter_km = resolution.edge_len_km() * 2.0;
    let k = (max_span_km / hex_diameter_km / 2.0 + 2.0) as u32;

    k.clamp(1, 50)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_cells_nonzero_for_small_bbox() {
        let sw = GeoCoordinate::new(48.85, 2.34).unwrap();  // Paris SW
        let ne = GeoCoordinate::new(48.90, 2.40).unwrap();  // Paris NE
        let cells = viewport_cells(&sw, &ne, H3Resolution::R7);
        assert!(!cells.is_empty());
    }

    #[test]
    fn viewport_cells_degenerate_point_returns_at_least_one() {
        let sw = GeoCoordinate::new(40.7128, -74.0060).unwrap(); // NYC
        let ne = GeoCoordinate::new(40.7129, -74.0059).unwrap(); // ~1m apart
        let cells = viewport_cells(&sw, &ne, H3Resolution::R9);
        assert!(!cells.is_empty());
    }
}
