//! End-to-end test of the `notify`-driven watcher against a real file.

use std::{sync::Arc, time::Duration};

use infra_config::{spawn_watcher, InfrastructureConfig, ResilienceRegistry};

fn config(timeout_ms: u64) -> String {
    format!(
        r#"
[resilience]
default_profile = "standard"
[resilience.profiles.standard]
timeout = {{ duration_ms = {timeout_ms} }}
circuit_breaker = {{ failure_threshold = 5, success_threshold = 2, open_duration_ms = 30000, half_open_max_calls = 1 }}
retry = {{ max_attempts = 3, backoff = {{ kind = "exponential", base_ms = 50, max_ms = 10000, jitter = "full" }} }}
"#
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_change_triggers_hot_reload() {
    // Unique temp dir so parallel test runs don't collide.
    let dir = std::env::temp_dir().join(format!("resilience-cfg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("infrastructure.toml");
    std::fs::write(&path, config(10_000)).unwrap();

    let registry = Arc::new(
        ResilienceRegistry::from_config(
            InfrastructureConfig::from_toml(&std::fs::read_to_string(&path).unwrap()).unwrap(),
        )
        .unwrap(),
    );
    let standard = registry.profile("standard").unwrap();
    assert_eq!(standard.timeout.load().duration.as_millis(), 10_000);

    // Keep the guard alive for the duration of the test.
    let _watcher = spawn_watcher(path.clone(), Arc::clone(&registry)).unwrap();

    // Mutate the file; the watcher should pick it up and swap the live handle.
    std::fs::write(&path, config(750)).unwrap();

    let mut reloaded = false;
    for _ in 0..100 {
        if standard.timeout.load().duration.as_millis() == 750 {
            reloaded = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let _ = std::fs::remove_dir_all(&dir);
    assert!(reloaded, "file change should have hot-reloaded the live profile within 5s");
}
