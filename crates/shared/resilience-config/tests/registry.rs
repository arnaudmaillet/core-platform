use resilience_config::{InfrastructureConfig, ResilienceRegistry};

const SAMPLE: &str = r#"
[resilience]
default_profile = "standard"

[resilience.profiles.standard]
timeout = { duration_ms = 10000 }
circuit_breaker = { failure_threshold = 5, success_threshold = 2, open_duration_ms = 30000, half_open_max_calls = 1 }
retry = { max_attempts = 3, backoff = { kind = "exponential", base_ms = 50, max_ms = 10000, jitter = "full" } }

[resilience.profiles.critical]
timeout = { duration_ms = 2000 }
circuit_breaker = { failure_threshold = 5, success_threshold = 2, open_duration_ms = 30000, half_open_max_calls = 1 }
retry = { max_attempts = 1, backoff = { kind = "exponential", base_ms = 20, max_ms = 500, jitter = "full" } }

[resilience.bindings]
"post-command" = "critical"
"timeline-read" = "standard"
"#;

fn registry_from(toml: &str) -> ResilienceRegistry {
    ResilienceRegistry::from_config(InfrastructureConfig::from_toml(toml).unwrap()).unwrap()
}

#[test]
fn resolves_bindings_and_default() {
    let registry = registry_from(SAMPLE);

    // explicit binding
    assert_eq!(registry.profile_for("post-command").timeout.load().duration.as_millis(), 2000);
    // explicit binding to standard
    assert_eq!(registry.profile_for("timeline-read").timeout.load().duration.as_millis(), 10000);
    // unbound dependency -> default_profile ("standard")
    assert_eq!(registry.profile_for("some-unbound-dep").timeout.load().duration.as_millis(), 10000);

    // resolved (non-hot-reloadable) retry comes through too
    assert_eq!(registry.profile("critical").unwrap().retry.max_attempts, 1);
}

#[test]
fn apply_hot_swaps_contents_for_existing_profiles() {
    let registry = registry_from(SAMPLE);
    let critical = registry.profile_for("post-command");
    assert_eq!(critical.timeout.load().duration.as_millis(), 2000);

    // A tighter config push (e.g. SRE mitigating an incident).
    let tightened = SAMPLE.replace("duration_ms = 2000", "duration_ms = 500");
    registry
        .apply(InfrastructureConfig::from_toml(&tightened).unwrap())
        .unwrap();

    // The previously-resolved handle observes the swap — no rebuild, no restart.
    assert_eq!(critical.timeout.load().duration.as_millis(), 500);
}

#[test]
fn rejects_binding_to_unknown_profile() {
    let bad = r#"
[resilience]
default_profile = "standard"
[resilience.profiles.standard]
timeout = { duration_ms = 1000 }
circuit_breaker = { failure_threshold = 1, success_threshold = 1, open_duration_ms = 1000, half_open_max_calls = 1 }
retry = { max_attempts = 1, backoff = { kind = "exponential", base_ms = 10, max_ms = 100 } }
[resilience.bindings]
"x" = "does-not-exist"
"#;
    let err = ResilienceRegistry::from_config(InfrastructureConfig::from_toml(bad).unwrap());
    assert!(err.is_err(), "binding to unknown profile must be rejected");
}

#[test]
fn rejects_inverted_backoff_bounds() {
    let bad = r#"
[resilience]
default_profile = "standard"
[resilience.profiles.standard]
timeout = { duration_ms = 1000 }
circuit_breaker = { failure_threshold = 1, success_threshold = 1, open_duration_ms = 1000, half_open_max_calls = 1 }
retry = { max_attempts = 1, backoff = { kind = "exponential", base_ms = 1000, max_ms = 100 } }
"#;
    let err = ResilienceRegistry::from_config(InfrastructureConfig::from_toml(bad).unwrap());
    assert!(err.is_err(), "max_ms < base_ms must be rejected");
}

#[test]
fn apply_rejects_invalid_config_and_keeps_previous() {
    let registry = registry_from(SAMPLE);
    let before = registry.profile_for("post-command").timeout.load().duration.as_millis();

    // failure_threshold = 0 is invalid; apply must fail-closed.
    let invalid = SAMPLE.replace("failure_threshold = 5", "failure_threshold = 0");
    let result = registry.apply(InfrastructureConfig::from_toml(&invalid).unwrap());

    assert!(result.is_err(), "invalid config must be rejected");
    assert_eq!(
        registry.profile_for("post-command").timeout.load().duration.as_millis(),
        before,
        "values must be unchanged after a rejected apply"
    );
}
