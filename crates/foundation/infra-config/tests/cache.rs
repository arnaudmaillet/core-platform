//! Cache section: catalog resolution, hot-swap, and fail-closed validation.

use infra_config::{CacheRegistry, InfrastructureConfig};

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
[cache.profiles.hot]
ttl_secs = 60
negative_ttl_secs = 10
[cache.bindings]
"handle-lookup" = "hot"
"#;

fn cache_registry(toml: &str) -> CacheRegistry {
    let cfg = InfrastructureConfig::from_toml(toml).unwrap();
    CacheRegistry::from_section(cfg.cache.expect("[cache] present")).unwrap()
}

#[test]
fn resolves_bindings_and_default() {
    let registry = cache_registry(SAMPLE);

    // explicit binding -> hot
    assert_eq!(registry.profile_for("handle-lookup").ttl().as_secs(), 60);
    assert_eq!(
        registry.profile_for("handle-lookup").negative_ttl().map(|d| d.as_secs()),
        Some(10)
    );

    // unbound namespace -> default_profile ("standard"), negative caching off
    assert_eq!(registry.profile_for("profile-view").ttl().as_secs(), 300);
    assert_eq!(registry.profile_for("profile-view").negative_ttl(), None);
}

#[test]
fn apply_hot_swaps_ttl_for_existing_profiles() {
    let registry = cache_registry(SAMPLE);
    let hot = registry.profile_for("handle-lookup");
    assert_eq!(hot.ttl().as_secs(), 60);

    // SRE widens the hot TTL to ride out a stampede.
    let widened = SAMPLE.replace("ttl_secs = 60", "ttl_secs = 600");
    let cfg = InfrastructureConfig::from_toml(&widened).unwrap();
    registry.apply(cfg.cache.unwrap()).unwrap();

    // The previously-resolved handle observes the swap — no rebuild, no restart.
    assert_eq!(hot.ttl().as_secs(), 600);
}

#[test]
fn rejects_zero_ttl() {
    let bad = SAMPLE.replace("ttl_secs = 300", "ttl_secs = 0");
    let cfg = InfrastructureConfig::from_toml(&bad).unwrap();
    let err = CacheRegistry::from_section(cfg.cache.unwrap()).err().expect("expected error");
    assert!(err.to_string().contains("ttl_secs must be > 0"), "got: {err}");
}

#[test]
fn rejects_binding_to_unknown_profile() {
    let bad = SAMPLE.replace(r#""handle-lookup" = "hot""#, r#""handle-lookup" = "nope""#);
    let cfg = InfrastructureConfig::from_toml(&bad).unwrap();
    let err = CacheRegistry::from_section(cfg.cache.unwrap()).err().expect("expected error");
    assert!(err.to_string().contains("unknown profile 'nope'"), "got: {err}");
}

#[test]
fn cache_section_is_optional() {
    let resilience_only = r#"
[resilience]
default_profile = "standard"
[resilience.profiles.standard]
timeout = { duration_ms = 10000 }
circuit_breaker = { failure_threshold = 5, success_threshold = 2, open_duration_ms = 30000, half_open_max_calls = 1 }
retry = { max_attempts = 3, backoff = { kind = "exponential", base_ms = 50, max_ms = 10000, jitter = "full" } }
"#;
    let cfg = InfrastructureConfig::from_toml(resilience_only).unwrap();
    assert!(cfg.cache.is_none());
    cfg.validate().unwrap();
}
