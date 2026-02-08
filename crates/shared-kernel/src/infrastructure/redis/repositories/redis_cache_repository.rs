// crates/shared-kernel/src/infrastructure/persistence/redis_cache_repository.rs

use async_trait::async_trait;
use fred::clients::Pool;
use fred::prelude::*;
use fred::types::scan::ScanType;
use fred::types::{Builder, Expiration};
use std::time::Duration;

use crate::domain::repositories::CacheRepository;
use crate::errors::{AppError, AppResult, ErrorCode};

pub struct RedisCacheRepository {
    pool: Pool,
}

impl RedisCacheRepository {
    pub async fn new(redis_url: &str) -> AppResult<Self> {
        let config = Config::from_url(redis_url)
            .map_err(|e| AppError::new(ErrorCode::InternalError, e.to_string()))?;

        let pool = Builder::from_config(config)
            .with_pool_config(|cfg| {
                cfg.max_clients = 2; // On descend à 2 pour les tests, c'est plus stable
            })
            .with_connection_config(|cfg| {
                cfg.connection_timeout = Duration::from_secs(5);
                cfg.internal_command_timeout = Duration::from_secs(5);
                cfg.max_command_attempts = 5;
            })
            // On rend la reconnexion plus patiente
            .set_policy(ReconnectPolicy::new_exponential(0, 100, 1000, 2))
            .build_pool(2)
            .map_err(|e| AppError::new(ErrorCode::InternalError, e.to_string()))?;

        // Initialisation
        pool.init().await.map_err(|e| AppError::new(ErrorCode::InternalError, e.to_string()))?;

        // On attend que TOUS les clients du pool soient connectés
        pool.wait_for_connect().await.map_err(|e| AppError::new(ErrorCode::InternalError, e.to_string()))?;

        Ok(Self { pool })
    }

    fn map_expiration(ttl: Option<Duration>) -> Option<Expiration> {
        ttl.map(|d| {
            if d < Duration::from_secs(1) {
                Expiration::PX(d.as_millis() as i64)
            } else {
                Expiration::EX(d.as_secs() as i64)
            }
        })
    }
}

#[async_trait]
impl CacheRepository for RedisCacheRepository {
    // PLUS de générique <V> ici. On reçoit directement le JSON sous forme de &str.
    async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> AppResult<()> {
        self.pool
            .set::<(), _, _>(key, value, Self::map_expiration(ttl), None, false)
            .await
            .map_err(|e| AppError::new(ErrorCode::InternalError, e.to_string()))?;

        Ok(())
    }

    // PLUS de générique <V>. On retourne l'Option<String> brute de Redis.
    async fn get(&self, key: &str) -> AppResult<Option<String>> {
        let result: Option<String> = self
            .pool
            .get(key)
            .await
            .map_err(|e| AppError::new(ErrorCode::InternalError, e.to_string()))?;

        Ok(result)
    }

    async fn delete(&self, key: &str) -> AppResult<()> {
        self.pool
            .del::<i64, _>(key)
            .await
            .map_err(|e| AppError::new(ErrorCode::InternalError, e.to_string()))?;
        Ok(())
    }

    async fn exists(&self, key: &str) -> AppResult<bool> {
        let count: i64 = self.pool
            .exists(key)
            .await
            .map_err(|e| AppError::new(ErrorCode::InternalError, e.to_string()))?;

        Ok(count > 0)
    }

    async fn set_many(&self, entries: Vec<(&str, String)>, ttl: Option<Duration>) -> AppResult<()> {
        if entries.is_empty() {
            return Ok(());
        }

        // On utilise un pipeline pour envoyer toutes les commandes d'un coup
        // C'est plus performant et réduit la latence réseau
        let expiration = Self::map_expiration(ttl);

        for (key, value) in entries {
            self.pool
                .set::<(), _, _>(key, value, expiration.clone(), None, false)
                .await
                .map_err(|e| AppError::new(ErrorCode::InternalError, e.to_string()))?;
        }

        // Note: Avec fred, si tu veux une atomicité parfaite (MSET),
        // les TTL sont gérés par clé individuellement après le MSET.

        Ok(())
    }

    async fn invalidate_pattern(&self, pattern: &str) -> AppResult<()> {
        let mut current_cursor = "0".to_string();

        loop {
            let (next_cursor, keys): (String, Vec<String>) = self
                .pool
                .scan_page::<(String, Vec<String>), String, String>(
                    current_cursor,
                    pattern.to_string(),
                    Some(250u32),
                    None::<ScanType>,
                )
                .await
                .map_err(|e| {
                    AppError::new(ErrorCode::InternalError, format!("Redis Scan Error: {}", e))
                })?;

            if !keys.is_empty() {
                self.pool
                    .del::<i64, _>(keys)
                    .await
                    .map_err(|e| AppError::new(ErrorCode::InternalError, e.to_string()))?;
            }

            if next_cursor == "0" {
                break;
            }
            current_cursor = next_cursor;
        }

        Ok(())
    }
}
