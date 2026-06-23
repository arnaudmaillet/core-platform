//! `[telemetry]` tenant: boot apply, hot-reload, and fail-closed via a fake control.

use std::sync::{Arc, Mutex};

use infra_config::{InfraRegistry, InfrastructureConfig, Reloadable, TelemetryControl};

/// Records applied dials; optionally rejects log filters containing a marker substring
/// (stands in for telemetry's real parse-check).
#[derive(Default)]
struct FakeControl {
    filters: Mutex<Vec<String>>,
    ratios: Mutex<Vec<f64>>,
    reject_marker: Mutex<Option<String>>,
}

impl TelemetryControl for FakeControl {
    fn validate_filter(&self, directives: &str) -> Result<(), String> {
        match self.reject_marker.lock().unwrap().as_deref() {
            Some(marker) if directives.contains(marker) => Err(format!("invalid filter: {directives}")),
            _ => Ok(()),
        }
    }
    fn set_filter(&self, directives: &str) -> Result<(), String> {
        self.filters.lock().unwrap().push(directives.to_owned());
        Ok(())
    }
    fn set_sampling_ratio(&self, ratio: f64) {
        self.ratios.lock().unwrap().push(ratio);
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
sampling_ratio = 0.1
"#;

fn registry(control: Arc<FakeControl>) -> InfraRegistry {
    let cfg = InfrastructureConfig::from_toml(BASE).unwrap();
    let control: Arc<dyn TelemetryControl> = control;
    InfraRegistry::from_config(cfg).unwrap().with_telemetry_control(control).unwrap()
}

#[test]
fn boot_applies_both_dials() {
    let control = Arc::new(FakeControl::default());
    let _reg = registry(Arc::clone(&control));
    assert_eq!(control.filters.lock().unwrap().as_slice(), &["info,post=debug".to_string()]);
    assert_eq!(control.ratios.lock().unwrap().as_slice(), &[0.1]);
}

#[test]
fn hot_reload_applies_new_dials() {
    let control = Arc::new(FakeControl::default());
    let reg = registry(Arc::clone(&control));

    let next = BASE.replace("info,post=debug", "warn,post=trace").replace("0.1", "1.0");
    reg.reload(&next).unwrap();

    assert_eq!(control.filters.lock().unwrap().last().unwrap(), "warn,post=trace");
    assert_eq!(*control.ratios.lock().unwrap().last().unwrap(), 1.0);
}

#[test]
fn out_of_range_ratio_rejected_at_validation() {
    let bad = BASE.replace("sampling_ratio = 0.1", "sampling_ratio = 1.5");
    // Range is validated in infra-config (no control needed) — fails at parse/from_config.
    let cfg = InfrastructureConfig::from_toml(&bad).unwrap();
    let err = InfraRegistry::from_config(cfg).err().expect("expected error");
    assert!(err.to_string().contains("must be in [0.0, 1.0]"), "got: {err}");
}

#[test]
fn bad_filter_rejects_whole_reload() {
    let control = Arc::new(FakeControl::default());
    *control.reject_marker.lock().unwrap() = Some("BADFILTER".to_string());
    let reg = registry(Arc::clone(&control));

    let filters_before = control.filters.lock().unwrap().len();
    let ratios_before = control.ratios.lock().unwrap().len();
    let bad = BASE.replace("info,post=debug", "BADFILTER");
    let err = reg.reload(&bad).unwrap_err();

    assert!(err.to_string().contains("invalid filter"), "got: {err}");
    // Fail-closed: neither dial was applied for the rejected push.
    assert_eq!(control.filters.lock().unwrap().len(), filters_before);
    assert_eq!(control.ratios.lock().unwrap().len(), ratios_before);
}

#[test]
fn dials_are_individually_optional() {
    let sampling_only = r#"
[resilience]
default_profile = "standard"
[resilience.profiles.standard]
timeout = { duration_ms = 10000 }
circuit_breaker = { failure_threshold = 5, success_threshold = 2, open_duration_ms = 30000, half_open_max_calls = 1 }
retry = { max_attempts = 3, backoff = { kind = "exponential", base_ms = 50, max_ms = 10000, jitter = "full" } }

[telemetry]
sampling_ratio = 0.5
"#;
    let control = Arc::new(FakeControl::default());
    let cfg = InfrastructureConfig::from_toml(sampling_only).unwrap();
    let dyn_control: Arc<dyn TelemetryControl> = Arc::clone(&control) as _;
    let _reg = InfraRegistry::from_config(cfg).unwrap().with_telemetry_control(dyn_control).unwrap();

    // Only the sampling dial was set; no filter was touched.
    assert!(control.filters.lock().unwrap().is_empty());
    assert_eq!(control.ratios.lock().unwrap().as_slice(), &[0.5]);
}
