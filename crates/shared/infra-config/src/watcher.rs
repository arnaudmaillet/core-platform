//! `notify`-based hot-reload: a single-writer task that re-applies the config on file change.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::{error::ConfigError, reload::Reloadable, schema::InfrastructureConfig};

/// Reads, parses, and validates (all sections) a config file in one shot.
pub fn load_from_path(path: &Path) -> Result<InfrastructureConfig, ConfigError> {
    let raw = std::fs::read_to_string(path)?;
    let config = InfrastructureConfig::from_toml(&raw)?;
    config.validate()?;
    Ok(config)
}

/// Starts watching `path` and re-applies the config to `target` on every change.
///
/// Generic over any [`Reloadable`]: pass an [`InfraRegistry`](crate::InfraRegistry) to drive
/// every section, or a [`ResilienceRegistry`](crate::ResilienceRegistry) for the standalone
/// resilience-only deployment — the watcher never sees a section's shape.
///
/// Returns the [`RecommendedWatcher`] guard — **keep it alive**; dropping it stops the watch.
///
/// Design notes:
/// * **Single writer.** All swaps happen in one spawned task, so reloads never race.
/// * **Fail-closed.** Parse/validation errors are logged and the previous config is kept;
///   a bad push can't take the fleet down.
/// * **K8s-aware.** ConfigMaps are mounted via an atomically-swapped `..data` symlink, which
///   replaces the file's inode rather than editing it. We therefore watch the *parent
///   directory*, not the file path, or we'd stop receiving events after the first swap.
/// * **Coalesced.** Editors and atomic swaps emit bursts; we drain the channel and reload once.
pub fn spawn_watcher<R: Reloadable>(
    path: PathBuf,
    target: Arc<R>,
) -> Result<RecommendedWatcher, ConfigError> {
    // Bridge notify's synchronous callback into the async world via an unbounded channel
    // (the payload is just a "something changed" tick; we re-read the file on the other side).
    let (tx, mut rx) = mpsc::unbounded_channel::<()>();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| match res {
        Ok(event) if is_relevant(&event.kind) => {
            let _ = tx.send(());
        }
        Ok(_) => {}
        Err(e) => error!(error = %e, "resilience config watch error"),
    })?;

    let watch_dir = path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
    watcher.watch(&watch_dir, RecursiveMode::NonRecursive)?;

    tokio::spawn(async move {
        while rx.recv().await.is_some() {
            // Drain any events that piled up behind this one so a burst triggers one reload.
            while rx.try_recv().is_ok() {}

            match std::fs::read_to_string(&path) {
                // `reload` parses + validates + swaps, fail-closed, behind the single writer.
                Ok(raw) => match target.reload(&raw) {
                    Ok(()) => info!(path = %path.display(), "infrastructure config hot-reloaded"),
                    Err(e) => {
                        warn!(error = %e, "rejected reloaded infrastructure config — keeping previous")
                    }
                },
                Err(e) => {
                    warn!(error = %e, "failed to read infrastructure config — keeping previous")
                }
            }
        }
    });

    Ok(watcher)
}

/// File create/modify/remove are the events that can change config contents; access/other
/// events are ignored to avoid needless reloads.
fn is_relevant(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    )
}
