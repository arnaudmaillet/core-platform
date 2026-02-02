// crates/shared-kernel/src/infrastructure/concurrency/singleflight.rs

//! # Singleflight - Déduplication de requêtes concurrentes
//!
//! Ce module implémente le pattern **Singleflight**, dont le but est de garantir qu'une seule
//! instance d'une opération asynchrone est en cours d'exécution pour une clé donnée.
//!
//! ### Cas d'usage principal : Le "Thundering Herd"
//! Lorsqu'un cache expire ou qu'une ressource coûteuse est demandée par 1000 utilisateurs
//! simultanément, au lieu de lancer 1000 appels à la base de données ou au microservice distant,
//! `Singleflight` va :
//! 1. Exécuter l'appel pour le premier utilisateur.
//! 2. Faire attendre les 999 autres sur le même résultat.
//! 3. Distribuer le résultat final à tout le monde une fois l'appel terminé.
//!
//! ### Avantages
//! - Réduit drastiquement la charge sur l'infrastructure (DB, API externes).
//! - Prévient les pics de latence lors de l'expiration de caches.
//! - Utilise `DashMap` pour une gestion thread-safe et performante des requêtes en cours.

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
        use dashmap::mapref::entry::Entry;

        // On utilise Entry pour faire un Check-and-Insert ATOMIQUE
        let shared_fut = match self.requests.entry(key.clone()) {
            Entry::Occupied(entry) => {
                // Déjà un leader en cours, on récupère son futur partagé
                entry.get().clone()
            }
            Entry::Vacant(entry) => {
                // On est le leader !
                let (tx, rx) = oneshot::channel();
                let shared_rx = rx.shared();
                entry.insert(shared_rx.clone());

                // On lance le travail réel
                // Note : On sort du verrou DashMap avant le .await pour ne pas bloquer les autres
                let result = factory().await;

                // On diffuse le résultat
                let _ = tx.send(result.clone());

                // Nettoyage immédiat après le travail
                self.requests.remove(&key);

                return result;
            }
        };

        // Si on arrive ici, on est un "suiveur", on attend le résultat du leader
        match shared_fut.await {
            Ok(result) => result,
            Err(_) => Err(AppError::new(
                ErrorCode::InternalError,
                "Singleflight leader panicked or dropped".to_string(),
            )),
        }
    }
}