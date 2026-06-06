// crates/geo_discovery/src/application/workers/cache_eviction_worker.rs

use chrono::Utc;
use shared_kernel::core::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

use crate::application::context::GeoDiscoveryAppContext;

pub struct CacheEvictionWorker {
    app_ctx: Arc<GeoDiscoveryAppContext>,
    execution_interval: Duration,
}

impl CacheEvictionWorker {
    pub fn new(app_ctx: Arc<GeoDiscoveryAppContext>, execution_interval: Duration) -> Self {
        Self {
            app_ctx,
            execution_interval,
        }
    }

    pub async fn start(self) {
        let mut timer = interval(self.execution_interval);
        tracing::info!(
            "Cache Eviction Worker démarré (Fréquence : {:?})",
            self.execution_interval
        );

        loop {
            timer.tick().await;

            if let Err(err) = self.run_eviction_cycle().await {
                tracing::error!("Erreur lors du cycle d'éviction du cache géo : {:?}", err);
            }
        }
    }

    /// Exécute un cycle complet de nettoyage chirurgical
    async fn run_eviction_cycle(&self) -> Result<()> {
        tracing::debug!("Début du cycle d'éviction des posts périmés...");

        let cache_repo = self.app_ctx.cache_repo();

        // 1. Récupérer uniquement les tuiles qui ont du contenu actif (évite le scan global de Redis)
        let active_tiles = cache_repo.get_all_active_tiles().await?;

        if active_tiles.is_empty() {
            tracing::debug!("Aucune tuile active enregistrée. Rien à nettoyer.");
            return Ok(());
        }

        let now = Utc::now();
        let mut total_evicted = 0;
        let mut tiles_cleaned_up = 0;

        // 2. Parcourir uniquement les zones ciblées
        for (resolution, tile_id) in active_tiles {
            // Expulsion des posts expirés dans la tuile (ZREMRANGEBYSCORE sous le capot)
            let evicted_ids = cache_repo
                .evict_old_posts(resolution, &tile_id, now)
                .await?;
            total_evicted += evicted_ids.len();

            // 3. Sécurité anti-fuite de mémoire : Est-ce que la tuile est devenue vide ?
            let current_count = cache_repo.get_tile_post_count(resolution, &tile_id).await?;

            if current_count == 0 {
                // Plus aucun post actif dans cette zone ! On la raye de l'index global
                cache_repo.untrack_active_tile(resolution, &tile_id).await?;
                tiles_cleaned_up += 1;
            }
        }

        // Logging des performances du cycle
        if total_evicted > 0 || tiles_cleaned_up > 0 {
            tracing::info!(
                "Cycle d'éviction terminé : {} posts expirés supprimés, {} tuiles devenues inactives et nettoyées de l'index.",
                total_evicted,
                tiles_cleaned_up
            );
        }

        Ok(())
    }
}
