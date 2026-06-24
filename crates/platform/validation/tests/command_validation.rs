mod common;

use cqrs::{CommandBus, CqrsError, Envelope, MiddlewarePipeline};
use uuid::Uuid;
use validation::{ValidationError, ValidationLayer, VAL_1001_REQUIRED, VAL_1002_LENGTH, VAL_1004_RANGE};

use common::{
    AlwaysInvalidCommand, AlwaysValidCommand, InlineCommandBus, MultiViolationCommand,
};

// ── helpers ───────────────────────────────────────────────────────────────────

fn correlation() -> Uuid {
    Uuid::now_v7()
}

// ── valid command reaches the inner bus ───────────────────────────────────────

#[tokio::test]
async fn valid_command_passes_through_to_inner_bus() {
    let stub = InlineCommandBus::new();
    let bus = MiddlewarePipeline::new(stub.clone())
        .layer(ValidationLayer)
        .build();

    let envelope = Envelope::new(correlation(), AlwaysValidCommand { value: "hello".into() });
    let result = bus.dispatch(envelope).await;

    assert!(result.is_ok(), "expected Ok(()), got: {result:?}");
    assert!(
        stub.was_reached(),
        "inner bus must be reached for a valid command"
    );
}

// ── invalid command is short-circuited before the inner bus ──────────────────

#[tokio::test]
async fn invalid_command_short_circuits_before_inner_bus() {
    let stub = InlineCommandBus::new();
    let bus = MiddlewarePipeline::new(stub.clone())
        .layer(ValidationLayer)
        .build();

    let envelope = Envelope::new(correlation(), AlwaysInvalidCommand { username: String::new() });
    let result = bus.dispatch(envelope).await;

    assert!(result.is_err(), "expected Err, got Ok(())");
    assert!(
        !stub.was_reached(),
        "inner bus must NOT be reached for an invalid command"
    );
}

// ── error shape: HTTP 422 and correct error code ──────────────────────────────

#[tokio::test]
async fn invalid_command_returns_cqrs_handler_error_with_val_code() {
    use error::AppError;

    let stub = InlineCommandBus::new();
    let bus = MiddlewarePipeline::new(stub)
        .layer(ValidationLayer)
        .build();

    let envelope = Envelope::new(correlation(), AlwaysInvalidCommand { username: String::new() });
    let err = bus.dispatch(envelope).await.unwrap_err();

    assert!(
        matches!(err, CqrsError::Handler(_)),
        "error must be CqrsError::Handler, got: {err:?}"
    );
    assert_eq!(err.error_code(), "VAL-0001");
    assert_eq!(err.http_status(), http::StatusCode::UNPROCESSABLE_ENTITY);
    assert!(!err.is_retryable());
    assert_eq!(err.category(), "VALIDATION");
}

// ── single violation: field, code, and message are preserved ─────────────────

#[tokio::test]
async fn single_violation_field_and_code_are_preserved() {
    let stub = InlineCommandBus::new();
    let bus = MiddlewarePipeline::new(stub)
        .layer(ValidationLayer)
        .build();

    let envelope = Envelope::new(correlation(), AlwaysInvalidCommand { username: String::new() });
    let err = bus.dispatch(envelope).await.unwrap_err();

    // Downcast the CqrsError handler payload to inspect the ValidationError.
    // We verify through the Display impl which encodes field, code, and message.
    let display = format!("{err}");
    assert!(
        display.contains("VAL-1001"),
        "display must contain VAL-1001, got: {display}"
    );
    assert!(
        display.contains("username"),
        "display must contain field name 'username', got: {display}"
    );
}

// ── multiple violations are fully aggregated ──────────────────────────────────

#[tokio::test]
async fn multiple_violations_are_all_reported() {
    let stub = InlineCommandBus::new();
    let bus = MiddlewarePipeline::new(stub)
        .layer(ValidationLayer)
        .build();

    let envelope = Envelope::new(correlation(), MultiViolationCommand);
    let err = bus.dispatch(envelope).await.unwrap_err();

    let display = format!("{err}");
    assert!(display.contains(VAL_1001_REQUIRED), "must contain VAL-1001");
    assert!(display.contains(VAL_1002_LENGTH),   "must contain VAL-1002");
    assert!(display.contains(VAL_1004_RANGE),    "must contain VAL-1004");
    assert!(display.contains("username"), "must mention 'username' field");
    assert!(display.contains("bio"),      "must mention 'bio' field");
    assert!(display.contains("age"),      "must mention 'age' field");
}

// ── details map shape ─────────────────────────────────────────────────────────

#[tokio::test]
async fn validation_error_details_map_contains_all_fields() {
    // Build ValidationError directly to test the details-map logic
    // without going through the bus — isolates the error type behaviour.
    use validate_core::FieldViolation;

    let err = ValidationError::new(vec![
        FieldViolation::new("username", VAL_1001_REQUIRED, "must not be empty"),
        FieldViolation::new("bio",      VAL_1002_LENGTH,   "must be at most 160 characters"),
    ]);

    let map = err.to_details_map();

    assert_eq!(map.len(), 2);
    assert!(
        map["username"].contains(VAL_1001_REQUIRED),
        "username entry must contain the VAL-1001 code"
    );
    assert!(
        map["bio"].contains(VAL_1002_LENGTH),
        "bio entry must contain the VAL-1002 code"
    );
}

// ── ValidationLayer composes with other layers ────────────────────────────────

#[tokio::test]
async fn validation_layer_composes_with_logging_and_tracing_layers() {
    use cqrs::{LoggingLayer, TracingLayer};

    let stub = InlineCommandBus::new();
    let bus = MiddlewarePipeline::new(stub.clone())
        .layer(ValidationLayer)
        .layer(TracingLayer)
        .layer(LoggingLayer)
        .build();

    // Valid command still reaches the inner bus through the full stack.
    let result = bus
        .dispatch(Envelope::new(correlation(), AlwaysValidCommand { value: "ok".into() }))
        .await;
    assert!(result.is_ok());
    assert!(stub.was_reached());
}
