use chrono::{Datelike, TimeZone, Utc};
use geo_discovery::types::BucketHour;
use shared_kernel::core::{ErrorCode, ValueObject};

#[test]
fn test_bucket_hour_truncates_to_start_of_day() {
    // Fixons une date précise : 15 Mai 2026 à 14:30:15 UTC
    let dt = Utc.with_ymd_and_hms(2026, 5, 15, 14, 30, 15).unwrap();
    let ts_millis = dt.timestamp_millis();

    // Le bucket attendu doit être le 15 Mai 2026 à 00:00:00 UTC
    let expected_dt = Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap();
    let expected_ts = expected_dt.timestamp_millis();

    let bucket = BucketHour::from_timestamp(ts_millis);

    assert_eq!(bucket.value(), expected_ts);
    assert_eq!(bucket.to_date_time(), expected_dt);
}

#[test]
fn test_bucket_hour_boundaries_same_day() {
    // Testons les deux extrêmes de la même journée (15 Mai 2026)
    let start_of_day = Utc
        .with_ymd_and_hms(2026, 5, 15, 0, 0, 0)
        .unwrap()
        .timestamp_millis();
    let end_of_day = Utc
        .with_ymd_and_hms(2026, 5, 15, 23, 59, 59)
        .unwrap()
        .timestamp_millis()
        + 999;

    let bucket_start = BucketHour::from_timestamp(start_of_day);
    let bucket_end = BucketHour::from_timestamp(end_of_day);

    // Les deux doivent pointer exactement sur le même bucket racine
    assert_eq!(bucket_start, bucket_end);
    assert_eq!(bucket_start.to_date_time().day(), 15); // MODIFIÉ : Utilise maintenant .day() du trait Datelike
}

#[test]
fn test_bucket_hour_consecutive_days_separation() {
    // On s'assure qu'une milliseconde d'écart à la frontière change de bucket
    let day_1_extreme = Utc
        .with_ymd_and_hms(2026, 5, 15, 23, 59, 59)
        .unwrap()
        .timestamp_millis()
        + 999;
    let day_2_start = Utc
        .with_ymd_and_hms(2026, 5, 16, 0, 0, 0)
        .unwrap()
        .timestamp_millis();

    let bucket_1 = BucketHour::from_timestamp(day_1_extreme);
    let bucket_2 = BucketHour::from_timestamp(day_2_start);

    assert_ne!(bucket_1, bucket_2);
    assert_eq!(
        bucket_2.value() - bucket_1.value(),
        BucketHour::MILLIS_IN_DAY
    );
}

#[test]
fn test_value_object_validation_invariants() {
    // 1. Un bucket valide (date contemporaine)
    let valid_ts = Utc
        .with_ymd_and_hms(2026, 5, 15, 0, 0, 0)
        .unwrap()
        .timestamp_millis();
    let valid_bucket = BucketHour::from_timestamp(valid_ts);
    assert!(valid_bucket.validate().is_ok());

    // 2. Un bucket invalide (égal à zéro / Epoch origin)
    let zero_bucket = BucketHour::from_timestamp(0);
    assert!(zero_bucket.validate().is_err());

    // 3. Un bucket invalide (négatif / avant 1970)
    let negative_bucket = BucketHour::from_timestamp(-12345);
    let err = negative_bucket.validate().unwrap_err();
    assert_eq!(err.code, ErrorCode::ValidationFailed);
}

#[test]
fn test_serde_serialization_integrity() {
    let dt = Utc.with_ymd_and_hms(2026, 5, 15, 12, 0, 0).unwrap();
    let bucket = BucketHour::from_timestamp(dt.timestamp_millis());

    // Test de sérialisation JSON (Serde)
    let serialized = serde_json::to_string(&bucket).expect("Serialization failed");

    let expected_json = format!("{}", bucket.value());
    assert_eq!(serialized, expected_json);

    // Test de désérialisation
    let deserialized: BucketHour =
        serde_json::from_str(&serialized).expect("Deserialization failed");
    assert_eq!(bucket, deserialized);
}
