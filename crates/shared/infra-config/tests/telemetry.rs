//! `[telemetry]` tenant: boot apply, hot-reload, and fail-closed via a fake control.

use std::sync::{Arc, Mutex};

use infra_config::{InfraRegistry, InfrastructureConfig, LogFilterControl, Reloadable};

/// Records applied filters; optionally rejects any directive containing a marker substring
/// (stands in for telemetry's real parse-check).
#[derive(Default)]
struct FakeControl {
    applied: Mutex<Vec<String>>,
    reject_marker: Mutex<Option<String>>,
}

impl LogFilterControl for FakeControl {
    fn validate_filter(&self, directives: &str) -> Result<(), String> {
        match self.reject_marker.lock().unwrap().as_deref() {
            Some(marker) if directives.contains(marker) => Err(format!("invalid filter: {directives}")),
            _ => Ok(()),
        }
    }
    fn set_filter(&self, directives: &str) -> Result<(), String> {
        self.applied.lock().unwrap().push(directives.to_owned());
        Ok(())
    }
}

const BASE: &str = r#"
[resilience]
default_profile = "standard"
[resilience.profiles.standard]
timeout = { duration_ms = 10000 }
circuit_breaker = { failure_threshold = 5, success_threshold = 2, open_duration_ms = 30000, half_open_max_calls = 1 }
retry = { max_attempts = 3, backoff = { kind = "exponential", base_ms = 50, max_ms = 10000, jitter = "full" } }

[telemetry]
log_filter = "info,post=debug"
"#;

fn registry(control: Arc<FakeControl>) -> InfraRegistry {
    let cfg = InfrastructureConfig::from_toml(BASE).unwrap();
    let control: Arc<dyn LogFilterControl> = control;
    InfraRegistry::from_config(cfg).unwrap().with_log_control(control).unwrap()
}

#[test]
fn boot_applies_configured_filter() {
    let control = Arc::new(FakeControl::default());
    let _reg = registry(Arc::clone(&control));
    assert_eq!(control.applied.lock().unwrap().as_slice(), &["info,post=debug".to_string()]);
}

#[test]
fn hot_reload_applies_new_filter() {
    let control = Arc::new(FakeControl::default());
    let reg = registry(Arc::clone(&control));

    let next = BASE.replace("info,post=debug", "warn,post=trace");
    reg.reload(&next).unwrap();

    assert_eq!(control.applied.lock().unwrap().last().unwrap(), "warn,post=trace");
}

#[test]
fn bad_filter_rejects_whole_reload() {
    let control = Arc::new(FakeControl::default());
    *control.reject_marker.lock().unwrap() = Some("BADFILTER".to_string());
    let reg = registry(Arc::clone(&control));

    let applied_before = control.applied.lock().unwrap().len();
    let bad = BASE.replace("info,post=debug", "BADFILTER");
    let err = reg.reload(&bad).unwrap_err();

    assert!(err.to_string().contains("invalid filter"), "got: {err}");
    // Fail-closed: set_filter was never called for the rejected push.
    assert_eq!(control.applied.lock().unwrap().len(), applied_before);
}

#[test]
fn telemetry_section_is_optional() {
    let resilience_only = r#"
[resilience]
default_profile = "standard"
[resilience.profiles.standard]
timeout = { duration_ms = 10000 }
circuit_breaker = { failure_threshold = 5, success_threshold = 2, open_duration_ms = 30000, half_open_max_calls = 1 }
retry = { max_attempts = 3, backoff = { kind = "exponential", base_ms = 50, max_ms = 10000, jitter = "full" } }
"#;
    let control = Arc::new(FakeControl::default());
    let cfg = InfrastructureConfig::from_toml(resilience_only).unwrap();
    let dyn_control: Arc<dyn LogFilterControl> = Arc::clone(&control) as _;
    let _reg = InfraRegistry::from_config(cfg).unwrap().with_log_control(dyn_control).unwrap();
    // No [telemetry] section → nothing applied at boot.
    assert!(control.applied.lock().unwrap().is_empty());
}
