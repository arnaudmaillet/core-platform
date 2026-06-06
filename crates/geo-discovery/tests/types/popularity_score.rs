use geo_discovery::types::PopularityScore;
use shared_kernel::core::ErrorCode;

#[test]
fn test_popularity_score_valid_creation() {
    let score = PopularityScore::try_new(42.5).expect("Should create a valid score");
    assert_eq!(score.value(), 42.5);
}

#[test]
fn test_popularity_score_sanitizes_negative_values() {
    // Un score négatif à l'initialisation doit être redressé à 0.0 de manière transparente
    let score =
        PopularityScore::try_new(-10.5).expect("Should successfully sanitize negative values");
    assert_eq!(score.value(), 0.0);
}

#[test]
fn test_popularity_score_invalid_floats_rejection() {
    // Test du NaN (Not a Number)
    let res_nan = PopularityScore::try_new(f64::NAN);
    assert!(res_nan.is_err());
    assert_eq!(res_nan.unwrap_err().code, ErrorCode::ValidationFailed);

    // Test de l'infini positif
    let res_inf = PopularityScore::try_new(f64::INFINITY);
    assert!(res_inf.is_err());

    // Test de l'infini négatif
    let res_neg_inf = PopularityScore::try_new(f64::NEG_INFINITY);
    assert!(res_neg_inf.is_err());
}

#[test]
fn test_popularity_score_default_value() {
    let score = PopularityScore::default();
    assert_eq!(score.value(), 1.0);
}

#[test]
fn test_popularity_score_from_raw() {
    // from_raw doit contourner la sanitization et la validation (utile pour remonter l'infra)
    let score = PopularityScore::from_raw(-5.0);
    assert_eq!(score.value(), -5.0);
}

#[test]
fn test_apply_delta_success_variants() {
    let mut score = PopularityScore::try_new(10.0).unwrap();

    // 1. Ingestion d'un delta positif
    score.apply_delta(5.5).unwrap();
    assert_eq!(score.value(), 15.5);

    // 2. Ingestion d'un delta négatif standard
    score.apply_delta(-3.5).unwrap();
    assert_eq!(score.value(), 12.0);

    // 3. Décrémentation massive : le score doit être capé à 0.0 au lieu de devenir négatif
    score.apply_delta(-100.0).unwrap();
    assert_eq!(score.value(), 0.0);
}

#[test]
fn test_apply_delta_invalid_math_protection() {
    let mut score = PopularityScore::try_new(10.0).unwrap();

    // Tenter d'injecter un delta NaN doit échouer sans corrompre le score d'origine
    let res_nan = score.apply_delta(f64::NAN);
    assert!(res_nan.is_err());
    assert_eq!(score.value(), 10.0); // Préservation de l'état

    // Tenter de pousser au débordement (Infinity)
    let res_inf = score.apply_delta(f64::INFINITY);
    assert!(res_inf.is_err());
    assert_eq!(score.value(), 10.0);
}

#[test]
fn test_popularity_score_display_formatting() {
    let score_simple = PopularityScore::try_new(10.5).unwrap();
    assert_eq!(format!("{}", score_simple), "10.5000");

    let score_complex = PopularityScore::try_new(3.14159265).unwrap();
    assert_eq!(format!("{}", score_complex), "3.1416"); // Arrondi correct à la 4ème décimale
}
