mod common;

use error::{AppError, Severity};
use http::StatusCode;
use scylla_storage::error::ScyllaStorageError;

// ─────────────────────────────────────────────────────────────────────────────
// Retryability matrix
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn transient_errors_are_retryable() {
    let retryable: &[ScyllaStorageError] = &[
        ScyllaStorageError::WriteTimeout {
            consistency: "LocalQuorum".into(),
            received: 1,
            required: 2,
        },
        ScyllaStorageError::ReadTimeout {
            consistency: "LocalOne".into(),
            received: 0,
            required: 1,
        },
        ScyllaStorageError::Unavailable {
            consistency: "Quorum".into(),
            required: 3,
            alive: 1,
        },
        ScyllaStorageError::Overloaded,
        ScyllaStorageError::RateLimitReached,
        ScyllaStorageError::IsBootstrapping,
        ScyllaStorageError::ClientTimeout { millis: 2_000 },
        ScyllaStorageError::ConnectionPool {
            message: "pool empty".into(),
        },
        ScyllaStorageError::Transport {
            message: "broken pipe".into(),
        },
    ];

    for err in retryable {
        assert!(
            err.is_retryable(),
            "{} should be retryable",
            err.error_code()
        );
    }
}

#[test]
fn non_transient_errors_are_not_retryable() {
    let permanent: &[ScyllaStorageError] = &[
        ScyllaStorageError::AuthenticationError {
            message: "bad password".into(),
        },
        ScyllaStorageError::Unauthorized {
            message: "no permission".into(),
        },
        ScyllaStorageError::AlreadyExists {
            keyspace: "ks".into(),
            table: "t".into(),
        },
        ScyllaStorageError::BadQuery {
            message: "syntax error".into(),
        },
        ScyllaStorageError::QueryInvalid {
            message: "unknown column".into(),
        },
        ScyllaStorageError::WriteFailure { numfailures: 1 },
        ScyllaStorageError::ReadFailure { numfailures: 1 },
        ScyllaStorageError::SchemaConflict {
            message: "schema drift".into(),
        },
        ScyllaStorageError::Bootstrap {
            message: "no host".into(),
        },
        ScyllaStorageError::Configuration {
            message: "empty dc".into(),
        },
        ScyllaStorageError::ProtocolError {
            message: "bad frame".into(),
        },
        ScyllaStorageError::Unknown {
            code: "0x1234".into(),
            message: "mystery".into(),
        },
    ];

    for err in permanent {
        assert!(
            !err.is_retryable(),
            "{} should NOT be retryable",
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

    let all: Vec<ScyllaStorageError> = vec![
        ScyllaStorageError::WriteTimeout {
            consistency: "LocalQuorum".into(),
            received: 1,
            required: 2,
        },
        ScyllaStorageError::ReadTimeout {
            consistency: "LocalOne".into(),
            received: 0,
            required: 1,
        },
        ScyllaStorageError::Unavailable {
            consistency: "Quorum".into(),
            required: 3,
            alive: 1,
        },
        ScyllaStorageError::Overloaded,
        ScyllaStorageError::RateLimitReached,
        ScyllaStorageError::IsBootstrapping,
        ScyllaStorageError::ClientTimeout { millis: 2_000 },
        ScyllaStorageError::ConnectionPool {
            message: String::new(),
        },
        ScyllaStorageError::Transport {
            message: String::new(),
        },
        ScyllaStorageError::AuthenticationError {
            message: String::new(),
        },
        ScyllaStorageError::Unauthorized {
            message: String::new(),
        },
        ScyllaStorageError::AlreadyExists {
            keyspace: String::new(),
            table: String::new(),
        },
        ScyllaStorageError::BadQuery {
            message: String::new(),
        },
        ScyllaStorageError::QueryInvalid {
            message: String::new(),
        },
        ScyllaStorageError::WriteFailure { numfailures: 0 },
        ScyllaStorageError::ReadFailure { numfailures: 0 },
        ScyllaStorageError::SchemaConflict {
            message: String::new(),
        },
        ScyllaStorageError::Bootstrap {
            message: String::new(),
        },
        ScyllaStorageError::Configuration {
            message: String::new(),
        },
        ScyllaStorageError::ProtocolError {
            message: String::new(),
        },
        ScyllaStorageError::Unknown {
            code: String::new(),
            message: String::new(),
        },
    ];

    let codes: HashSet<&'static str> = all.iter().map(|e| e.error_code()).collect();
    assert_eq!(
        codes.len(),
        all.len(),
        "duplicate error codes detected in ScyllaStorageError"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Severity alignment
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn critical_errors_have_correct_severity() {
    let critical: &[ScyllaStorageError] = &[
        ScyllaStorageError::Unavailable {
            consistency: "Quorum".into(),
            required: 2,
            alive: 0,
        },
        ScyllaStorageError::AuthenticationError {
            message: String::new(),
        },
        ScyllaStorageError::Bootstrap {
            message: String::new(),
        },
        ScyllaStorageError::Configuration {
            message: String::new(),
        },
        ScyllaStorageError::ProtocolError {
            message: String::new(),
        },
        ScyllaStorageError::BadQuery {
            message: String::new(),
        },
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

// ─────────────────────────────────────────────────────────────────────────────
// HTTP status codes
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn unavailable_maps_to_503() {
    let err = ScyllaStorageError::Unavailable {
        consistency: "LocalQuorum".into(),
        required: 2,
        alive: 0,
    };
    assert_eq!(err.http_status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[test]
fn unauthorized_maps_to_403() {
    let err = ScyllaStorageError::Unauthorized {
        message: "deny".into(),
    };
    assert_eq!(err.http_status(), StatusCode::FORBIDDEN);
}

#[test]
fn already_exists_maps_to_409() {
    let err = ScyllaStorageError::AlreadyExists {
        keyspace: "social".into(),
        table: "users".into(),
    };
    assert_eq!(err.http_status(), StatusCode::CONFLICT);
}

// ─────────────────────────────────────────────────────────────────────────────
// Category constant
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn category_is_sdb() {
    let err = ScyllaStorageError::Overloaded;
    assert_eq!(err.category(), "SDB");
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require live cluster; skipped unless --include-ignored)
// ─────────────────────────────────────────────────────────────────────────────

/// Verifies that connecting to a non-existent host produces a Bootstrap error.
#[tokio::test]
#[ignore = "requires network access to a non-routable IP"]
async fn bad_contact_point_produces_bootstrap_error() {
    use scylla_storage::{ScyllaSessionBuilder, config::ScyllaConfig};

    common::init_tracing();
    let mut config = ScyllaConfig::default();
    config.contact_points = vec!["192.0.2.1:9042".into()]; // TEST-NET — never routable
    config.connect_timeout = std::time::Duration::from_secs(2);

    let result = ScyllaSessionBuilder::new(config).build().await;
    assert!(result.is_err());
    match result.err().unwrap() {
        ScyllaStorageError::Bootstrap { .. } | ScyllaStorageError::Configuration { .. } => {}
        other => panic!("unexpected error variant: {:?}", other),
    }
}
