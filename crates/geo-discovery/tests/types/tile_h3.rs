use std::str::FromStr;

use geo_discovery::types::TileH3;
use shared_kernel::core::ErrorCode;

#[test]
fn test_h3_tile_success_creation() {
    // Un index H3 valide typique (Résolution 7 ou 8 par exemple en hexadécimal)
    let valid_index = "871f1d48bffffff";

    let tile = TileH3::try_new(valid_index.to_string()).expect("Should create a valid H3Tile");
    assert_eq!(tile.value(), "871f1d48bffffff");
}

#[test]
fn test_h3_tile_normalization_behavior() {
    // L'index doit être nettoyé (trim) et passé en minuscules (lowercase)
    let dirty_index = "  871F1D48BFFFFFF \n";

    let tile = TileH3::try_new(dirty_index.to_string()).expect("Should normalize input");
    assert_eq!(tile.value(), "871f1d48bffffff"); // Strictement lowercase et sans espaces
}

#[test]
fn test_h3_tile_validation_failures() {
    let scenarios = vec![
        ("", "H3 tile string cannot be empty"),
        ("871f1d4", "Invalid hexadecimal H3 index length"), // Trop court
        ("871f1d48bfffffffff", "Invalid hexadecimal H3 index length"), // Trop long
        (
            "871f1d48bffffgfa",
            "H3 index must be a valid hexadecimal string",
        ), // 'g' n'est pas hex
    ];

    for (input, _reason) in scenarios {
        let res = TileH3::try_new(input.to_string());
        assert!(
            res.is_err(),
            "Input '{}' should have failed validation",
            input
        );

        let err = res.unwrap_err();
        assert_eq!(err.code, ErrorCode::ValidationFailed);
    }
}

#[test]
fn test_h3_tile_from_str_and_try_from_traits() {
    let valid_index = "8a1f1d48bffffff";

    // Test de FromStr
    let tile_from_str = TileH3::from_str(valid_index).expect("FromStr conversion failed");
    assert_eq!(tile_from_str.value(), valid_index);

    // Test de TryFrom<String>
    let tile_try_from =
        TileH3::try_from(valid_index.to_string()).expect("TryFrom conversion failed");
    assert_eq!(tile_try_from, tile_from_str);

    // Test de From<H3Tile> pour String
    let output_string: String = tile_from_str.into();
    assert_eq!(output_string, valid_index);
}

#[test]
fn test_h3_tile_display_trait() {
    let tile = TileH3::try_new("871f1d48bffffff".to_string()).unwrap();
    let display_string = format!("{}", tile);
    assert_eq!(display_string, "871f1d48bffffff");
}

#[test]
fn test_h3_tile_serde_proxy_serialization() {
    let tile = TileH3::try_new("871f1d48bffffff".to_string()).unwrap();

    // Sérialisation JSON : comme il y a #[serde(into = "String")],
    // l'objet doit se sérialiser comme une chaîne JSON standard, pas une structure ou un tuple.
    let serialized = serde_json::to_string(&tile).expect("Serialization failed");
    assert_eq!(serialized, r#""871f1d48bffffff""#);

    // Désérialisation JSON : passe par #[serde(try_from = "String")]
    let deserialized: TileH3 = serde_json::from_str(&serialized).expect("Deserialization failed");
    assert_eq!(deserialized, tile);
}

#[test]
fn test_h3_tile_serde_deserialization_failure() {
    // Tenter de désérialiser une chaîne invalide via serde
    let invalid_json_str = r#""invalid_h3_index""#;

    let res: std::result::Result<TileH3, _> = serde_json::from_str(invalid_json_str);
    assert!(res.is_err());
}
