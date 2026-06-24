//! InfraRegistry: aggregated resolution + cross-section fail-closed apply.

use infra_config::{InfraRegistry, InfrastructureConfig, Reloadable};

const SAMPLE: &str = r#"
[resilience]
default_profile = "standard"
[resilience.profiles.standard]
timeout = { duration_ms = 10000 }
circuit_breaker = { failure_threshold = 5, success_threshold = 2, open_duration_ms = 30000, half_open_max_calls = 1 }
retry = { max_attempts = 3, backoff = { kind = "exponential", base_ms = 50, max_ms = 10000, jitter = "full" } }

[cache]
default_profile = "standard"
[cache.profiles.standard]
ttl_secs = 300
"#;

fn registry(toml: &str) -> InfraRegistry {
    InfraRegistry::from_config(InfrastructureConfig::from_toml(toml).unwrap()).unwrap()
}

#[test]
fn exposes_every_section() {
    let reg = registry(SAMPLE);
    assert_eq!(
        reg.resilience().profile_for("anything").timeout.load().duration.as_millis(),
        10000
    );
    let cache = reg.cache().expect("[cache] configured");
    assert_eq!(cache.profile_for("anything").ttl().as_secs(), 300);
}

#[test]
fn reload_swaps_all_sections_at_once() {
    let reg = registry(SAMPLE);
    let cache = reg.cache().unwrap();
    let resil = reg.resilience();
    let timeout = resil.profile_for("x");
    let ttl = cache.profile_for("x");

    let next = SAMPLE
        .replace("duration_ms = 10000", "duration_ms = 500")
        .replace("ttl_secs = 300", "ttl_secs = 30");
    reg.reload(&next).unwrap();

    assert_eq!(timeout.timeout.load().duration.as_millis(), 500);
    assert_eq!(ttl.ttl().as_secs(), 30);
}

#[test]
fn bad_section_rejects_whole_reload_leaving_all_untouched() {
    let reg = registry(SAMPLE);
    let cache = reg.cache().unwrap();
    let resil = reg.resilience();
    let timeout = resil.profile_for("x");
    let ttl = cache.profile_for("x");

    // Valid resilience change, but the cache section is now invalid (zero TTL).
    let bad = SAMPLE
        .replace("duration_ms = 10000", "duration_ms = 500")
        .replace("ttl_secs = 300", "ttl_secs = 0");
    let err = reg.reload(&bad).unwrap_err();
    assert!(err.to_string().contains("ttl_secs must be > 0"), "got: {err}");

    // All-or-nothing: neither section moved, including the otherwise-valid resilience one.
    assert_eq!(timeout.timeout.load().duration.as_millis(), 10000);
    assert_eq!(ttl.ttl().as_secs(), 300);
}
