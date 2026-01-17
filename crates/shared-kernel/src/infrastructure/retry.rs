// crates/shared-kernel/src/infrastructure/retry.rs

use rand::Rng;
use crate::errors::{DomainError, Result};

pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_backoff_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff_ms: 20,
        }
    }
}

/// Exécute une action avec une stratégie de retry (Exponential Backoff + Jitter).
/// Utile afin d'éviter le "Thundering Herd" lors de conflits de concurrence.
pub async fn with_retry<F, Fut, T>(config: RetryConfig, mut action: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    for attempt in 0..=config.max_retries {
        match action().await {
            Ok(res) => return Ok(res),
            Err(e) if e.is_concurrency_conflict() && attempt < config.max_retries => {
                // Calcul de l'exponentiel : 2^attempt * base
                let base_backoff = config.initial_backoff_ms * 2u64.pow(attempt);

                // Ajout du Jitter (entre 0 et 25% de la base) pour désynchroniser les clients
                let jitter = rand::rng().random_range(0..base_backoff / 4 + 1);

                let backoff = std::time::Duration::from_millis(base_backoff + jitter);

                tracing::warn!(
                    "🔄 Concurrency conflict (attempt {}/{}), retrying in {:?}...",
                    attempt + 1,
                    config.max_retries,
                    backoff
                );

                tokio::time::sleep(backoff).await;
                continue;
            }
            Err(e) => return Err(e),
        }
    }

    Err(DomainError::TooManyConflicts(
        format!("Operation failed after {} retries due to persistent conflicts", config.max_retries)
    ))
}