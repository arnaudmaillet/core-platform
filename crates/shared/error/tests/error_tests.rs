use std::fmt;

use error::{
    into_api_response, AppError, ApiErrorResponse, DistributedError, ErrorContext,
    IntoApiResponse, Severity,
};
use http::StatusCode;

// ── Fixtures ──────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
enum TestError {
    NotFound,
    Internal,
    Transient,
}

impl fmt::Display for TestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TestError::NotFound => write!(f, "not found"),
            TestError::Internal => write!(f, "internal error"),
            TestError::Transient => write!(f, "transient failure"),
        }
    }
}

impl std::error::Error for TestError {}

impl AppError for TestError {
    fn error_code(&self) -> &'static str {
        match self {
            TestError::NotFound => "TEST_NOT_FOUND",
            TestError::Internal => "TEST_INTERNAL",
            TestError::Transient => "TEST_TRANSIENT",
        }
    }

    fn http_status(&self) -> StatusCode {
        match self {
            TestError::NotFound => StatusCode::NOT_FOUND,
            TestError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            TestError::Transient => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    fn severity(&self) -> Severity {
        match self {
            TestError::NotFound => Severity::Low,
            TestError::Internal => Severity::Critical,
            TestError::Transient => Severity::High,
        }
    }

    fn is_retryable(&self) -> bool {
        matches!(self, TestError::Transient)
    }

    fn category(&self) -> &'static str {
        "TEST"
    }

    fn user_facing_message(&self) -> &'static str {
        match self {
            TestError::NotFound => "Resource not found.",
            TestError::Internal => "An internal error occurred.",
            TestError::Transient => "Service temporarily unavailable.",
        }
    }
}

// Uses all AppError defaults to verify the fallback behaviour.
#[derive(Debug)]
struct MinimalError;

impl fmt::Display for MinimalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "minimal")
    }
}

impl std::error::Error for MinimalError {}

impl AppError for MinimalError {
    fn error_code(&self) -> &'static str {
        "MINIMAL"
    }

    fn http_status(&self) -> StatusCode {
        StatusCode::IM_A_TEAPOT
    }
}

fn make_ctx() -> ErrorContext {
    ErrorContext::new("test-service")
}

// ── Severity ──────────────────────────────────────────────────────────────────

#[test]
fn severity_should_page_only_critical_and_high() {
    assert!(Severity::Critical.should_page());
    assert!(Severity::High.should_page());
    assert!(!Severity::Medium.should_page());
    assert!(!Severity::Low.should_page());
    assert!(!Severity::Info.should_page());
}

#[test]
fn severity_log_level_mapping() {
    use tracing::Level;
    assert_eq!(Severity::Critical.log_level(), Level::ERROR);
    assert_eq!(Severity::High.log_level(), Level::ERROR);
    assert_eq!(Severity::Medium.log_level(), Level::WARN);
    assert_eq!(Severity::Low.log_level(), Level::INFO);
    assert_eq!(Severity::Info.log_level(), Level::DEBUG);
}

#[test]
fn severity_as_label() {
    assert_eq!(Severity::Critical.as_label(), "Critical");
    assert_eq!(Severity::High.as_label(), "High");
    assert_eq!(Severity::Medium.as_label(), "Medium");
    assert_eq!(Severity::Low.as_label(), "Low");
    assert_eq!(Severity::Info.as_label(), "Info");
}

#[test]
fn severity_display_matches_label() {
    for s in [
        Severity::Critical,
        Severity::High,
        Severity::Medium,
        Severity::Low,
        Severity::Info,
    ] {
        assert_eq!(s.to_string(), s.as_label());
    }
}

#[test]
fn severity_ord_follows_declaration_order() {
    // Critical is the first variant so it is the smallest.
    assert!(Severity::Critical < Severity::High);
    assert!(Severity::High < Severity::Medium);
    assert!(Severity::Medium < Severity::Low);
    assert!(Severity::Low < Severity::Info);
}

#[test]
fn severity_copy() {
    let a = Severity::Critical;
    let b = a; // Copy — a is still usable
    assert_eq!(a, b);
}

#[test]
fn severity_clone() {
    let a = Severity::High;
    assert_eq!(a.clone(), a);
}

#[test]
fn severity_serialize_as_label_string() {
    assert_eq!(
        serde_json::to_string(&Severity::Critical).unwrap(),
        "\"Critical\""
    );
    assert_eq!(
        serde_json::to_string(&Severity::High).unwrap(),
        "\"High\""
    );
    assert_eq!(
        serde_json::to_string(&Severity::Medium).unwrap(),
        "\"Medium\""
    );
    assert_eq!(
        serde_json::to_string(&Severity::Low).unwrap(),
        "\"Low\""
    );
    assert_eq!(
        serde_json::to_string(&Severity::Info).unwrap(),
        "\"Info\""
    );
}

#[test]
fn severity_deserialize_roundtrip() {
    for s in [
        Severity::Critical,
        Severity::High,
        Severity::Medium,
        Severity::Low,
        Severity::Info,
    ] {
        let json = serde_json::to_string(&s).unwrap();
        let back: Severity = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}

// ── AppError trait ────────────────────────────────────────────────────────────

#[test]
fn app_error_trait_defaults() {
    let e = MinimalError;
    assert_eq!(e.severity(), Severity::Medium);
    assert!(!e.is_retryable());
    assert_eq!(e.category(), "UNKNOWN");
    assert_eq!(e.user_facing_message(), "An error occurred.");
}

#[test]
fn app_error_mandatory_fields() {
    let e = MinimalError;
    assert_eq!(e.error_code(), "MINIMAL");
    assert_eq!(e.http_status(), StatusCode::IM_A_TEAPOT);
}

#[test]
fn app_error_overridden_fields() {
    assert_eq!(TestError::Transient.severity(), Severity::High);
    assert!(TestError::Transient.is_retryable());
    assert_eq!(TestError::Transient.category(), "TEST");
    assert_eq!(
        TestError::Transient.user_facing_message(),
        "Service temporarily unavailable."
    );
    assert_eq!(TestError::Transient.error_code(), "TEST_TRANSIENT");
    assert_eq!(TestError::Transient.http_status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[test]
fn app_error_not_found_variant() {
    assert_eq!(TestError::NotFound.error_code(), "TEST_NOT_FOUND");
    assert_eq!(TestError::NotFound.http_status(), StatusCode::NOT_FOUND);
    assert_eq!(TestError::NotFound.severity(), Severity::Low);
    assert!(!TestError::NotFound.is_retryable());
}

#[test]
fn app_error_internal_variant() {
    assert_eq!(TestError::Internal.severity(), Severity::Critical);
    assert!(!TestError::Internal.is_retryable());
}

// ── IntoApiResponse blanket impl ──────────────────────────────────────────────

#[test]
fn into_api_response_trait_matches_from_error() {
    let e = TestError::NotFound;
    let ctx = make_ctx();
    let via_trait = e.to_api_response(&ctx);
    let via_fn = ApiErrorResponse::from_error(&e, &ctx);
    assert_eq!(
        serde_json::to_value(&via_trait).unwrap(),
        serde_json::to_value(&via_fn).unwrap(),
    );
}

#[test]
fn into_api_response_blanket_works_for_minimal_error() {
    let e = MinimalError;
    let ctx = make_ctx();
    let resp = e.to_api_response(&ctx);
    assert_eq!(resp.error_code, "MINIMAL");
    assert_eq!(resp.severity, Severity::Medium);
}

// ── ErrorContext ──────────────────────────────────────────────────────────────

#[test]
fn error_context_new_initial_state() {
    let ctx = ErrorContext::new("my-svc");
    assert_eq!(ctx.service_name, "my-svc");
    assert!(ctx.trace_id.is_none());
    assert!(ctx.span_id.is_none());
    assert!(ctx.metadata.is_empty());
    assert_ne!(
        ctx.request_id.to_string(),
        "00000000-0000-0000-0000-000000000000"
    );
}

#[test]
fn error_context_unique_request_ids() {
    let a = make_ctx();
    let b = make_ctx();
    assert_ne!(a.request_id, b.request_id);
}

#[test]
fn error_context_with_trace() {
    let ctx = ErrorContext::new("svc").with_trace("trace-abc", "span-xyz");
    assert_eq!(ctx.trace_id.as_deref(), Some("trace-abc"));
    assert_eq!(ctx.span_id.as_deref(), Some("span-xyz"));
}

#[test]
fn error_context_with_meta_chainable() {
    let ctx = ErrorContext::new("svc")
        .with_meta("user_id", "u_1")
        .with_meta("route", "/v1/foo");
    assert_eq!(ctx.metadata["user_id"], "u_1");
    assert_eq!(ctx.metadata["route"], "/v1/foo");
}

#[test]
fn error_context_full_builder_chain() {
    let ctx = ErrorContext::new("auth-service")
        .with_trace("t123", "s456")
        .with_meta("tenant", "acme")
        .with_meta("route", "POST /login");
    assert_eq!(ctx.service_name, "auth-service");
    assert_eq!(ctx.trace_id.as_deref(), Some("t123"));
    assert_eq!(ctx.span_id.as_deref(), Some("s456"));
    assert_eq!(ctx.metadata.len(), 2);
    assert_eq!(ctx.metadata["tenant"], "acme");
}

#[test]
fn error_context_clone_is_independent() {
    let ctx = ErrorContext::new("svc").with_meta("k", "v");
    let mut cloned = ctx.clone();
    cloned.metadata.insert("extra".into(), "val".into());
    // The original is not affected
    assert!(!ctx.metadata.contains_key("extra"));
}

#[test]
fn error_context_serialize_contains_expected_fields() {
    let ctx = ErrorContext::new("svc")
        .with_trace("t", "s")
        .with_meta("k", "v");
    let json: serde_json::Value = serde_json::to_value(&ctx).unwrap();
    assert_eq!(json["service_name"], "svc");
    assert_eq!(json["trace_id"], "t");
    assert_eq!(json["span_id"], "s");
    assert_eq!(json["metadata"]["k"], "v");
    assert!(json.get("request_id").is_some());
    assert!(json.get("timestamp").is_some());
}

#[test]
fn error_context_deserialize_from_static_json() {
    // service_name is &'static str, so deserialization requires a static string input.
    let json = r#"{
        "request_id": "550e8400-e29b-41d4-a716-446655440000",
        "trace_id": "trace-1",
        "span_id": "span-1",
        "service_name": "svc",
        "timestamp": "2024-01-15T10:30:00Z",
        "metadata": {"key": "value"}
    }"#;
    let ctx: ErrorContext = serde_json::from_str(json).unwrap();
    assert_eq!(ctx.service_name, "svc");
    assert_eq!(ctx.trace_id.as_deref(), Some("trace-1"));
    assert_eq!(ctx.span_id.as_deref(), Some("span-1"));
    assert_eq!(ctx.metadata["key"], "value");
}

#[test]
fn error_context_deserialize_null_trace_fields() {
    let json = r#"{
        "request_id": "550e8400-e29b-41d4-a716-446655440001",
        "trace_id": null,
        "span_id": null,
        "service_name": "svc",
        "timestamp": "2024-01-15T10:30:00Z",
        "metadata": {}
    }"#;
    let ctx: ErrorContext = serde_json::from_str(json).unwrap();
    assert!(ctx.trace_id.is_none());
    assert!(ctx.span_id.is_none());
}

// ── DistributedError ──────────────────────────────────────────────────────────

#[test]
fn distributed_error_stores_error_and_context() {
    let ctx = make_ctx();
    let rid = ctx.request_id;
    let de = DistributedError::new(TestError::Internal, ctx);
    assert!(matches!(de.error, TestError::Internal));
    assert_eq!(de.context.request_id, rid);
}

#[test]
fn distributed_error_display_contains_service_code_message_and_request_id() {
    let ctx = ErrorContext::new("my-svc");
    let rid = ctx.request_id;
    let de = DistributedError::new(TestError::NotFound, ctx);
    let s = de.to_string();
    assert!(s.contains("my-svc"), "missing service: {s}");
    assert!(s.contains("TEST_NOT_FOUND"), "missing error_code: {s}");
    assert!(s.contains("not found"), "missing error message: {s}");
    assert!(s.contains(&rid.to_string()), "missing request_id: {s}");
}

#[test]
fn distributed_error_source_downcasts_to_inner() {
    use std::error::Error;
    let de = DistributedError::new(TestError::Internal, make_ctx());
    let src = de.source().expect("source should be Some");
    let inner = src
        .downcast_ref::<TestError>()
        .expect("should downcast to TestError");
    assert_eq!(inner, &TestError::Internal);
}

#[test]
fn distributed_error_is_itself_an_error() {
    use std::error::Error;
    let de = DistributedError::new(TestError::NotFound, make_ctx());
    // DistributedError implements std::error::Error
    let _: &dyn Error = &de;
}

#[test]
fn distributed_error_log_does_not_panic_for_any_severity() {
    // Without a tracing subscriber, events are silently dropped — no panic.
    #[derive(Debug)]
    struct SeverityError(Severity);
    impl fmt::Display for SeverityError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "test")
        }
    }
    impl std::error::Error for SeverityError {}
    impl AppError for SeverityError {
        fn error_code(&self) -> &'static str {
            "S"
        }
        fn http_status(&self) -> StatusCode {
            StatusCode::OK
        }
        fn severity(&self) -> Severity {
            self.0
        }
    }

    for s in [
        Severity::Critical,
        Severity::High,
        Severity::Medium,
        Severity::Low,
        Severity::Info,
    ] {
        DistributedError::new(SeverityError(s), make_ctx()).log();
    }
}

// ── ApiErrorResponse / http ───────────────────────────────────────────────────

#[test]
fn api_error_response_maps_all_fields() {
    let ctx = ErrorContext::new("svc").with_meta("user_id", "u_42");
    let rid = ctx.request_id;
    let ts = ctx.timestamp;

    let resp = ApiErrorResponse::from_error(&TestError::Internal, &ctx);

    assert_eq!(resp.error_code, "TEST_INTERNAL");
    assert_eq!(resp.message, "An internal error occurred.");
    assert_eq!(resp.request_id, rid);
    assert_eq!(resp.service, "svc");
    assert_eq!(resp.severity, Severity::Critical);
    assert!(!resp.retryable);
    assert_eq!(resp.category, "TEST");
    assert_eq!(resp.timestamp, ts);
    assert_eq!(resp.details["user_id"], "u_42");
}

#[test]
fn api_error_response_no_trace_or_span_id() {
    let ctx = ErrorContext::new("svc").with_trace("trace-1", "span-1");
    let resp = ApiErrorResponse::from_error(&TestError::NotFound, &ctx);
    let json = serde_json::to_value(&resp).unwrap();
    assert!(json.get("trace_id").is_none(), "trace_id must not reach clients");
    assert!(json.get("span_id").is_none(), "span_id must not reach clients");
}

#[test]
fn api_error_response_retryable_field() {
    let ctx = make_ctx();
    assert!(!ApiErrorResponse::from_error(&TestError::NotFound, &ctx).retryable);
    assert!(ApiErrorResponse::from_error(&TestError::Transient, &ctx).retryable);
}

#[test]
fn api_error_response_details_mirror_metadata() {
    let ctx = ErrorContext::new("svc")
        .with_meta("k1", "v1")
        .with_meta("k2", "v2");
    let resp = ApiErrorResponse::from_error(&MinimalError, &ctx);
    assert_eq!(resp.details.len(), 2);
    assert_eq!(resp.details["k1"], "v1");
    assert_eq!(resp.details["k2"], "v2");
}

#[test]
fn api_error_response_empty_details_when_no_metadata() {
    let resp = ApiErrorResponse::from_error(&MinimalError, &make_ctx());
    assert!(resp.details.is_empty());
}

#[test]
fn into_api_response_fn_matches_from_error() {
    let de = DistributedError::new(TestError::NotFound, make_ctx());
    let via_fn = into_api_response(&de);
    let via_from = ApiErrorResponse::from_error(&de.error, &de.context);
    assert_eq!(
        serde_json::to_value(&via_fn).unwrap(),
        serde_json::to_value(&via_from).unwrap(),
    );
}

#[test]
fn api_error_response_json_shape() {
    let ctx = ErrorContext::new("auth-service");
    let rid = ctx.request_id;
    let resp = ApiErrorResponse::from_error(&TestError::NotFound, &ctx);
    let json: serde_json::Value = serde_json::to_value(&resp).unwrap();

    assert_eq!(json["error_code"], "TEST_NOT_FOUND");
    assert_eq!(json["message"], "Resource not found.");
    assert_eq!(json["service"], "auth-service");
    assert_eq!(json["severity"], "Low");
    assert_eq!(json["retryable"], false);
    assert_eq!(json["category"], "TEST");
    assert_eq!(json["request_id"], rid.to_string());
    assert!(json.get("timestamp").is_some());
    assert!(json.get("details").is_some());
}

#[test]
fn api_error_response_serde_roundtrip() {
    let ctx = ErrorContext::new("svc").with_meta("k", "v");
    let resp = ApiErrorResponse::from_error(&TestError::Transient, &ctx);
    let json = serde_json::to_string(&resp).unwrap();
    let back: ApiErrorResponse = serde_json::from_str(&json).unwrap();

    assert_eq!(back.error_code, resp.error_code);
    assert_eq!(back.request_id, resp.request_id);
    assert_eq!(back.severity, resp.severity);
    assert_eq!(back.retryable, resp.retryable);
    assert_eq!(back.category, resp.category);
    assert_eq!(back.service, resp.service);
    assert_eq!(back.details, resp.details);
    assert_eq!(back.message, resp.message);
}

#[test]
fn api_error_response_defaults_via_minimal_error() {
    let resp = ApiErrorResponse::from_error(&MinimalError, &make_ctx());
    assert_eq!(resp.error_code, "MINIMAL");
    assert_eq!(resp.message, "An error occurred.");
    assert_eq!(resp.severity, Severity::Medium);
    assert!(!resp.retryable);
    assert_eq!(resp.category, "UNKNOWN");
}
