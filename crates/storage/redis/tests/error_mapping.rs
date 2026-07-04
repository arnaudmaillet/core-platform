mod common;

use std::time::Duration;

use error::{AppError, Severity};
use http::StatusCode;
use redis_storage::RedisStorageError;

// ─────────────────────────────────────────────────────────────────────────────
// Retryability matrix
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn transient_errors_are_retryable() {
    let retryable: &[RedisStorageError] = &[
        RedisStorageError::Timeout      { message: "deadline exceeded".into() },
        RedisStorageError::Disconnected { message: "connection reset".into() },
        RedisStorageError::Io           { message: "broken pipe".into() },
        RedisStorageError::Backpressure,
        RedisStorageError::Canceled,
        RedisStorageError::PoolExhausted { message: "all connections in use".into() },
        RedisStorageError::Cluster      { message: "CLUSTERDOWN".into() },
        RedisStorageError::Sentinel     { message: "no primary found".into() },
    ];

    for err in retryable {
        assert!(
            err.is_retryable(),
            "{} (code: {}) should be retryable",
            err,
            err.error_code()
        );
    }
}

#[test]
fn permanent_errors_are_not_retryable() {
    let permanent: &[RedisStorageError] = &[
        RedisStorageError::Authentication { message: "WRONGPASS".into() },
        RedisStorageError::WrongType      { message: "WRONGTYPE".into() },
        RedisStorageError::InvalidArgument { message: "ERR value out of range".into() },
        RedisStorageError::InvalidCommand { message: "ERR unknown command".into() },
        RedisStorageError::NotFound,
        RedisStorageError::Configuration  { message: "empty host list".into() },
        RedisStorageError::Tls            { message: "certificate expired".into() },
        RedisStorageError::Protocol       { message: "unexpected byte".into() },
        RedisStorageError::Parse          { message: "expected integer".into() },
        RedisStorageError::Unknown        { message: "unhandled".into() },
    ];

    for err in permanent {
        assert!(
            !err.is_retryable(),
            "{} (code: {}) should NOT be retryable",
            err,
            err.error_code()
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Error code uniqueness
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn all_error_codes_are_unique() {
    use std::collections::HashSet;

    let all: Vec<RedisStorageError> = vec![
        RedisStorageError::Timeout       { message: String::new() },
        RedisStorageError::Disconnected  { message: String::new() },
        RedisStorageError::Io            { message: String::new() },
        RedisStorageError::Backpressure,
        RedisStorageError::Canceled,
        RedisStorageError::PoolExhausted { message: String::new() },
        RedisStorageError::Authentication { message: String::new() },
        RedisStorageError::WrongType     { message: String::new() },
        RedisStorageError::InvalidArgument { message: String::new() },
        RedisStorageError::InvalidCommand { message: String::new() },
        RedisStorageError::NotFound,
        RedisStorageError::Cluster       { message: String::new() },
        RedisStorageError::Sentinel      { message: String::new() },
        RedisStorageError::Configuration { message: String::new() },
        RedisStorageError::Tls           { message: String::new() },
        RedisStorageError::Protocol      { message: String::new() },
        RedisStorageError::Parse         { message: String::new() },
        RedisStorageError::Unknown       { message: String::new() },
    ];

    let codes: HashSet<&'static str> = all.iter().map(|e| e.error_code()).collect();
    assert_eq!(
        codes.len(),
        all.len(),
        "duplicate error codes detected in RedisStorageError"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Severity alignment
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn critical_errors_have_correct_severity() {
    let critical: &[RedisStorageError] = &[
        RedisStorageError::Authentication { message: String::new() },
        RedisStorageError::Configuration  { message: String::new() },
        RedisStorageError::Tls            { message: String::new() },
        RedisStorageError::Protocol       { message: String::new() },
    ];

    for err in critical {
        assert_eq!(
            err.severity(),
            Severity::Critical,
            "{} should be Critical",
            err.error_code()
        );
    }
}

#[test]
fn high_severity_transient_errors() {
    let high: &[RedisStorageError] = &[
        RedisStorageError::Timeout      { message: String::new() },
        RedisStorageError::Disconnected { message: String::new() },
        RedisStorageError::Io           { message: String::new() },
        RedisStorageError::Backpressure,
        RedisStorageError::PoolExhausted { message: String::new() },
        RedisStorageError::Cluster      { message: String::new() },
        RedisStorageError::Sentinel     { message: String::new() },
    ];

    for err in high {
        assert_eq!(
            err.severity(),
            Severity::High,
            "{} should be High",
            err.error_code()
        );
    }
}

#[test]
fn low_severity_client_errors() {
    let low: &[RedisStorageError] = &[
        RedisStorageError::WrongType      { message: String::new() },
        RedisStorageError::InvalidArgument { message: String::new() },
        RedisStorageError::NotFound,
    ];

    for err in low {
        assert_eq!(
            err.severity(),
            Severity::Low,
            "{} should be Low",
            err.error_code()
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// HTTP status codes
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn transient_errors_map_to_503() {
    let cases: &[RedisStorageError] = &[
        RedisStorageError::Timeout      { message: String::new() },
        RedisStorageError::Disconnected { message: String::new() },
        RedisStorageError::Io           { message: String::new() },
        RedisStorageError::Backpressure,
        RedisStorageError::PoolExhausted { message: String::new() },
        RedisStorageError::Cluster      { message: String::new() },
        RedisStorageError::Sentinel     { message: String::new() },
    ];

    for err in cases {
        assert_eq!(
            err.http_status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "{} should map to 503",
            err.error_code()
        );
    }
}

#[test]
fn not_found_maps_to_404() {
    assert_eq!(RedisStorageError::NotFound.http_status(), StatusCode::NOT_FOUND);
}

#[test]
fn wrong_type_maps_to_422() {
    let err = RedisStorageError::WrongType { message: "WRONGTYPE".into() };
    assert_eq!(err.http_status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[test]
fn authentication_maps_to_500() {
    let err = RedisStorageError::Authentication { message: "WRONGPASS".into() };
    assert_eq!(err.http_status(), StatusCode::INTERNAL_SERVER_ERROR);
}

// ─────────────────────────────────────────────────────────────────────────────
// Category constant
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn all_variants_report_rds_category() {
    let all: Vec<RedisStorageError> = vec![
        RedisStorageError::Timeout       { message: String::new() },
        RedisStorageError::Disconnected  { message: String::new() },
        RedisStorageError::Io            { message: String::new() },
        RedisStorageError::Backpressure,
        RedisStorageError::Canceled,
        RedisStorageError::PoolExhausted { message: String::new() },
        RedisStorageError::Authentication { message: String::new() },
        RedisStorageError::WrongType     { message: String::new() },
        RedisStorageError::InvalidArgument { message: String::new() },
        RedisStorageError::InvalidCommand { message: String::new() },
        RedisStorageError::NotFound,
        RedisStorageError::Cluster       { message: String::new() },
        RedisStorageError::Sentinel      { message: String::new() },
        RedisStorageError::Configuration { message: String::new() },
        RedisStorageError::Tls           { message: String::new() },
        RedisStorageError::Protocol      { message: String::new() },
        RedisStorageError::Parse         { message: String::new() },
        RedisStorageError::Unknown       { message: String::new() },
    ];

    for err in &all {
        assert_eq!(
            err.category(),
            "RDS",
            "expected RDS category for {}",
            err.error_code()
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// From<fred::error::Error> conversion — unit-level (no live server required)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn fred_timeout_error_converts_to_rds_1001() {
    use fred::error::{Error as FredError, ErrorKind};

    let fred_err = FredError::new(ErrorKind::Timeout, "deadline exceeded");
    let our_err  = RedisStorageError::from(fred_err);

    assert!(matches!(our_err, RedisStorageError::Timeout { .. }));
    assert_eq!(our_err.error_code(), "RDS-1001");
    assert!(our_err.is_retryable());
    assert_eq!(our_err.severity(), Severity::High);
    assert_eq!(our_err.http_status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[test]
fn fred_auth_error_converts_to_rds_3001() {
    use fred::error::{Error as FredError, ErrorKind};

    let fred_err = FredError::new(ErrorKind::Auth, "WRONGPASS invalid username-password pair");
    let our_err  = RedisStorageError::from(fred_err);

    assert!(matches!(our_err, RedisStorageError::Authentication { .. }));
    assert_eq!(our_err.error_code(), "RDS-3001");
    assert!(!our_err.is_retryable());
    assert_eq!(our_err.severity(), Severity::Critical);
}

#[test]
fn fred_cluster_error_converts_to_rds_5001() {
    use fred::error::{Error as FredError, ErrorKind};

    let fred_err = FredError::new(ErrorKind::Cluster, "CLUSTERDOWN");
    let our_err  = RedisStorageError::from(fred_err);

    assert!(matches!(our_err, RedisStorageError::Cluster { .. }));
    assert_eq!(our_err.error_code(), "RDS-5001");
    assert!(our_err.is_retryable());
}

#[test]
fn fred_config_error_converts_to_rds_8001() {
    use fred::error::{Error as FredError, ErrorKind};

    let fred_err = FredError::new(ErrorKind::Config, "empty host list");
    let our_err  = RedisStorageError::from(fred_err);

    assert!(matches!(our_err, RedisStorageError::Configuration { .. }));
    assert_eq!(our_err.error_code(), "RDS-8001");
    assert!(!our_err.is_retryable());
    assert_eq!(our_err.severity(), Severity::Critical);
}

#[test]
fn fred_not_found_converts_to_rds_4004() {
    use fred::error::{Error as FredError, ErrorKind};

    let fred_err = FredError::new(ErrorKind::NotFound, "key does not exist");
    let our_err  = RedisStorageError::from(fred_err);

    assert!(matches!(our_err, RedisStorageError::NotFound));
    assert_eq!(our_err.error_code(), "RDS-4004");
    assert!(!our_err.is_retryable());
    assert_eq!(our_err.http_status(), StatusCode::NOT_FOUND);
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require a live Redis instance; skipped unless --include-ignored)
// ─────────────────────────────────────────────────────────────────────────────

/// Verifies that connecting to a non-routable IP produces a `Disconnected`
/// or `Timeout` error when `fail_fast = true`.
#[tokio::test]
#[ignore = "requires network access to a non-routable IP"]
async fn unreachable_host_produces_disconnected_or_timeout_error() {
    common::setup::init_tracing();

    let mut config = RedisConfig::default();
    config.hosts              = vec!["192.0.2.1:6379".into()]; // TEST-NET — never routable
    config.connection_timeout = Duration::from_secs(2);
    config.command_timeout    = Duration::from_millis(2_000);
    config.fail_fast          = true;

    let result = RedisClientBuilder::new(config).build().await;
    assert!(result.is_err());

    match result.err().unwrap() {
        RedisStorageError::Disconnected { .. }
        | RedisStorageError::Timeout { .. }
        | RedisStorageError::Io { .. } => {}
        other => panic!("unexpected error variant: {:?}", other),
    }
}

use redis_storage::{RedisClientBuilder, RedisConfig};
