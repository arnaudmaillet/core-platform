// crates/geo_discovery/src/application/services/hydration_worker.rs

use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::use_cases::{HydrateTileCacheCommand, HydrateTileCacheHandler};
type InFlightKeys = Arc<DashMap<(i32, String), ()>>;

pub struct MapAnnotationCacheHydrationWorker {
    handler: Arc<HydrateTileCacheHandler>,
}

impl MapAnnotationCacheHydrationWorker {
    pub fn new(handler: HydrateTileCacheHandler) -> Self {
        Self {
            handler: Arc::new(handler),
        }
    }

    pub fn start(self, mut receiver: mpsc::Receiver<HydrateTileCacheCommand>) {
        tokio::spawn(async move {
            // DashMap thread-safe pour suivre l'état des tâches actives (Single Flight)
            let in_flight_registry: InFlightKeys = Arc::new(DashMap::new());

            while let Some(command) = receiver.recv().await {
                let handler_clone = self.handler.clone();
                let registry_clone = in_flight_registry.clone();

                // Clé unique pour identifier la tâche d'hydratation sur cette tuile
                let task_key = (command.resolution.value(), command.tile_id.to_string());

                // --- SÉCURITÉ ANTI-TEMPÊTE (Single Flight) ---
                // Si la tuile est déjà en train d'être calculée, on ignore silencieusement la commande
                if registry_clone.contains_key(&task_key) {
                    continue;
                }

                // On marque le début de l'hydratation
                registry_clone.insert(task_key.clone(), ());

                // On délègue l'exécution à un sous-worker Tokio dédié pour ne pas bloquer
                // la lecture des messages suivants de la file mpsc
                tokio::spawn(async move {
                    let tile_str = task_key.1.clone();
                    let res_val = task_key.0;

                    match handler_clone.handle(command).await {
                        Ok(_) => {
                            // Succès, la donnée est chaude dans Redis
                        }
                        Err(e) => {
                            // Logger l'erreur ici de manière industrielle via ton framework de log (tracing/log)
                            eprintln!(
                                "[Hydration Worker Error] Failed to hydrate tile {} at res {}: {}",
                                tile_str, res_val, e
                            );
                        }
                    }

                    // Travail terminé : On retire la tuile du registre in-flight
                    registry_clone.remove(&task_key);
                });
            }
        });
    }
}
