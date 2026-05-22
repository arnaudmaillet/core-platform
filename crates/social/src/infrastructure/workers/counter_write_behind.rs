use infra_fred::fred::{clients::Pool, interfaces::SetsInterface};
use shared_kernel::core::Result;
use shared_kernel::types::ProfileId;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{redis::RedisCounterRepository, repositories::CounterRepository};

pub struct CounterWriteBehindWorker {
    redis_pool: Pool,
    redis_counter_repo: Arc<RedisCounterRepository>,
    scylla_counter_repo: Arc<dyn CounterRepository>,
    batch_size: u32,
}

impl CounterWriteBehindWorker {
    pub fn new(
        redis_pool: Pool,
        redis_counter_repo: Arc<RedisCounterRepository>,
        scylla_counter_repo: Arc<dyn CounterRepository>,
        batch_size: u32,
    ) -> Self {
        Self {
            redis_pool,
            redis_counter_repo,
            scylla_counter_repo,
            batch_size,
        }
    }

    pub async fn start(self, tick_duration: Duration) {
        let mut timer = interval(tick_duration);
        println!(
            "[Worker] Counter Write-Back initialisé (Tick: {:?})",
            tick_duration
        );

        loop {
            timer.tick().await;

            if let Err(e) = self.process_batch().await {
                eprintln!(
                    "[Worker Error] Échec de la synchronisation des compteurs: {:?}",
                    e
                );
            }
        }
    }

    async fn process_batch(&self) -> Result<()> {
        let dirty_profile_strings: Vec<String> = self
            .redis_pool
            .spop("profiles:dirty", Some(self.batch_size as usize))
            .await
            .map_err(|e| shared_kernel::core::Error::internal(e.to_string()))?;

        if dirty_profile_strings.is_empty() {
            return Ok(());
        }

        info!(
            target: "counter_worker",
            "Traitement de {} profils modifiés...",
            dirty_profile_strings.len()
        );

        for profile_str in dirty_profile_strings {
            if let Ok(parsed_uuid) = Uuid::parse_str(&profile_str) {
                let profile_id = ProfileId::from(parsed_uuid);

                match self.redis_counter_repo.get_counters(profile_id).await {
                    Ok(counters) => {
                        if let Err(e) = self.scylla_counter_repo.save(&counters).await {
                            error!(
                                "Impossible de sauvegarder le profil {} dans ScyllaDB: {:?}",
                                profile_id, e
                            );
                            let _ = self
                                .redis_pool
                                .sadd::<i64, _, _>("profiles:dirty", profile_str.clone())
                                .await;
                        }
                    }
                    Err(e) => warn!(
                        "Erreur de lecture Redis pour le profil {}: {:?}",
                        profile_id, e
                    ),
                }
            } else {
                error!(
                    "Impossible de parser l'ID Redis '{}' en Uuid valide",
                    profile_str
                );
            }
        }

        Ok(())
    }
}
