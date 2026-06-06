use geo_discovery::types::MapViewport;
use h3o::Resolution;
use shared_kernel::core::ErrorCode;
use shared_proto::geo_discovery::v1::LatLng;

// Utilitaire pour forger un LatLng rapidement dans les tests
fn make_lat_lng(lat: f64, lng: f64) -> LatLng {
    LatLng {
        latitude: lat,
        longitude: lng,
    }
}

#[test]
fn test_map_viewport_valid_creation() {
    let sw = make_lat_lng(48.81, 2.31); // Paris Sud-Ouest
    let ne = make_lat_lng(48.90, 2.42); // Paris Nord-Est

    let viewport =
        MapViewport::try_new(sw.clone(), ne.clone()).expect("Should create a valid viewport");
    assert_eq!(viewport.south_west(), &sw);
    assert_eq!(viewport.north_east(), &ne);
}

#[test]
fn test_map_viewport_invalid_geometry() {
    let sw = make_lat_lng(49.0, 2.0);
    let ne = make_lat_lng(48.0, 3.0);

    let res = MapViewport::try_new(sw, ne);
    assert!(res.is_err());

    let err = res.unwrap_err();
    assert_eq!(err.code, ErrorCode::ValidationFailed);

    let details = err.details.expect("Error details should be populated");
    let reason = details
        .get("reason")
        .and_then(|v| v.as_str())
        .expect("Reason field should be a string");

    assert!(reason.contains("La latitude Sud-Ouest ne peut pas être supérieure"));
}

#[test]
fn test_get_intersecting_tiles_deduplication_and_validity() {
    // Zone centrée sur Paris
    let sw = make_lat_lng(48.8156, 2.3204);
    let ne = make_lat_lng(48.8989, 2.3847);
    let viewport = MapViewport::try_new(sw, ne).unwrap();

    // Échantillonnage à une résolution pivot large (Résolution 5)
    let tiles = viewport
        .get_intersecting_tiles(Resolution::Five)
        .expect("Discretization failed");

    assert!(!tiles.is_empty(), "Should return at least one H3 tile");

    // Test d'unicité stricte : s'assurer que le filtre `!tiles.contains(&tile)` a fonctionné
    let mut unique_check = std::collections::HashSet::new();
    for tile in &tiles {
        assert!(
            unique_check.insert(tile.value().to_string()),
            "Duplicate H3 tile detected in the viewport rendering output: {}",
            tile.value()
        );
    }
}

#[test]
fn test_get_intersecting_tiles_resolution_scaling() {
    // Utilisation d'un tout petit viewport (BBox restreinte)
    let sw = make_lat_lng(48.8566, 2.3522);
    let ne = make_lat_lng(48.8576, 2.3532);
    let viewport = MapViewport::try_new(sw, ne).unwrap();

    // 1. À basse résolution (Resolution::Four), la zone doit tenir dans une seule grosse tuile
    let low_res_tiles = viewport.get_intersecting_tiles(Resolution::Four).unwrap();

    // 2. À haute résolution (Resolution::Ten), le maillage doit capturer plusieurs micro-tuiles distinctes
    let high_res_tiles = viewport.get_intersecting_tiles(Resolution::Ten).unwrap();

    // Plus la résolution est élevée, plus le nombre de tuiles doit croître pour couvrir la même surface
    assert!(
        high_res_tiles.len() >= low_res_tiles.len(),
        "High resolution grids ({}) must yield more tiles than low resolution grids ({})",
        high_res_tiles.len(),
        low_res_tiles.len()
    );
}
