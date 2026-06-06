use geo_discovery::types::TileResolution;
use shared_kernel::core::ErrorCode;

#[test]
fn test_tile_resolution_valid_creation() {
    let res = TileResolution::try_new(7).expect("Should accept resolution 7");
    assert_eq!(res.value(), 7);

    // Test des bornes inclusives
    let min_res = TileResolution::try_new(0).expect("Should accept resolution 0");
    assert_eq!(min_res.value(), 0);

    let max_res = TileResolution::try_new(15).expect("Should accept resolution 15");
    assert_eq!(max_res.value(), 15);
}

#[test]
fn test_tile_resolution_out_of_bounds_rejection() {
    // En dessous de la limite minimale
    let res_under = TileResolution::try_new(-1);
    assert!(res_under.is_err());
    assert_eq!(res_under.unwrap_err().code, ErrorCode::ValidationFailed);

    // Au-dessus de la limite maximale (H3 s'arrête à 15)
    let res_over = TileResolution::try_new(16);
    assert!(res_over.is_err());
}

#[test]
fn test_from_client_zoom_mappings() {
    let scenarios = vec![
        (0.0, 3), // Macro-vue : Pays / Continent
        (3.9, 3), // Limite haute du premier palier
        (4.0, 5), // Transition vue régionale
        (6.9, 5),
        (7.0, 7), // Transition vue urbaine (Zone chaude)
        (10.9, 7),
        (11.0, 9), // Transition vue quartier
        (13.9, 9),
        (14.0, 10), // Transition vue rue / Hyper-locale (Plafond infra)
        (20.0, 10), // Zoom extrême : doit rester bloqué au plafond de sécurité (10)
    ];

    for (zoom, expected_resolution) in scenarios {
        let res = TileResolution::from_client_zoom(zoom);
        assert_eq!(
            res.value(),
            expected_resolution,
            "Zoom level {} should map to resolution {}",
            zoom,
            expected_resolution
        );
    }
}

#[test]
fn test_from_client_zoom_int_bridge() {
    // Simple redirection vers l'implémentation flottante
    let res = TileResolution::from_client_zoom_int(12);
    assert_eq!(res.value(), 9);
}

#[test]
fn test_conversion_traits() {
    // Test de TryFrom<i32>
    let res_try: TileResolution = TileResolution::try_from(5).expect("Conversion should succeed");
    assert_eq!(res_try.value(), 5);

    // Test de From<TileResolution> pour i32
    let raw_val: i32 = i32::from(res_try);
    assert_eq!(raw_val, 5);
}

#[test]
fn test_serde_serialization_proxy() {
    let res = TileResolution::try_new(7).unwrap();

    // Sérialisation JSON : doit produire un entier brut en sortie grâce à #[serde(into = "i32")]
    let serialized = serde_json::to_string(&res).expect("Serialization failed");
    assert_eq!(serialized, "7");

    // Désérialisation JSON : doit intercepter l'entier brut et exécuter le TryFrom
    let deserialized: TileResolution =
        serde_json::from_str(&serialized).expect("Deserialization failed");
    assert_eq!(deserialized, res);
}

#[test]
fn test_serde_deserialization_failure() {
    // Une valeur JSON hors limites doit faire échouer la désérialisation proprement
    let invalid_json = "16";
    let res: std::result::Result<TileResolution, _> = serde_json::from_str(invalid_json);
    assert!(res.is_err());
}
