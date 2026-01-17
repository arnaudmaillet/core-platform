// crates/shared-kernel/src/infrastructure/concurrency/singleflight.rs

use std::sync::Arc;
use dashmap::DashMap;
use futures::future::{Shared, FutureExt};
use std::future::Future;
use tokio::sync::oneshot;
use crate::errors::{AppResult, AppError, ErrorCode};

pub struct Singleflight<K, T> {
    requests: DashMap<K, Shared<oneshot::Receiver<AppResult<T>>>>,
}

impl<K, T> Singleflight<K, T>
where
    K: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            requests: DashMap::new(),
        }
    }

    pub async fn execute<F, Fut>(&self, key: K, factory: F) -> AppResult<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = AppResult<T>> + Send + 'static,
    {
        // 1. Tenter de récupérer une requête déjà en cours
        if let Some(shared_fut) = self.requests.get(&key) {
            return match shared_fut.value().clone().await {
                Ok(result) => result,
                Err(_) => Err(AppError::new(
                    ErrorCode::InternalError,
                    "Singleflight sender dropped".to_string()
                )),
            };
        }

        // 2. Préparer le canal de communication
        let (tx, rx) = oneshot::channel();
        let shared_rx = rx.shared();

        // On insère dans la map (on clone la version partagée)
        self.requests.insert(key.clone(), shared_rx.clone());

        // 3. Exécuter la factory (le travail réel)
        let result = factory().await;

        // 4. Envoyer le résultat à tous les Receiver en attente
        let _ = tx.send(result.clone());

        // 5. Nettoyage
        self.requests.remove(&key);

        result
    }
}