// crates/profile/src/infrastructure/postgres/repositories/postgres_location_repository.rs

use crate::domain::entities::UserLocation;
use crate::domain::repositories::LocationRepository;
use crate::infrastructure::postgres::rows::PostgresLocationRow;
use async_trait::async_trait;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::entities::GeoPoint;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::postgres::mappers::SqlxErrorExt;
use sqlx::PgPool;

pub struct PostgresLocationRepository {
    pool: PgPool,
}

impl PostgresLocationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LocationRepository for PostgresLocationRepository {
    async fn save(&self, loc: &UserLocation, tx: Option<&mut dyn Transaction>) -> Result<()> {
        let pool = self.pool.clone();
        let row = PostgresLocationRow::from(loc);

        // On calcule l'ancienne version attendue
        let current_version = row.version; // N + 1
        let old_version = current_version - 1; // N

        <dyn Transaction>::execute_on(&pool, tx, |conn| Box::pin(async move {
            // 1. Tentative d'UPDATE avec Optimistic Locking
            let update_sql = r#"
            UPDATE user_locations SET
                coordinates = ST_SetSRID(ST_MakePoint($3, $4), 4326)::geography,
                accuracy_meters = $5, altitude = $6, heading = $7, speed = $8,
                is_ghost_mode = $9, privacy_radius_meters = $10,
                updated_at = $11, version = $12
            WHERE account_id = $1 AND region_code = $2 AND version = $13
        "#;

            let result = sqlx::query(update_sql)
                .bind(row.account_id)            // $1
                .bind(&row.region_code)          // $2
                .bind(row.lon)                   // $3
                .bind(row.lat)                   // $4
                .bind(row.accuracy_meters)       // $5
                .bind(row.altitude)              // $6
                .bind(row.heading)               // $7
                .bind(row.speed)                 // $8
                .bind(row.is_ghost_mode)         // $9
                .bind(row.privacy_radius_meters) // $10
                .bind(row.updated_at)            // $11
                .bind(row.version)               // $12 (Nouvelle version)
                .bind(old_version)               // $13 (Ancienne version attendue)
                .execute(&mut *conn)
                .await
                .map_domain_infra("UserLocationUpdate")?;

            if result.rows_affected() == 0 {
                // 2. Si l'UPDATE échoue, soit ça n'existe pas, soit il y a un conflit
                let (exists,): (bool,) = sqlx::query_as(
                    "SELECT EXISTS(SELECT 1 FROM user_locations WHERE account_id = $1 AND region_code = $2)"
                )
                    .bind(row.account_id)
                    .bind(&row.region_code)
                    .fetch_one(&mut *conn)
                    .await
                    .map_domain_infra("UserLocationExistsCheck")?;

                if !exists {
                    // 3. INSERT initial
                    let insert_sql = r#"
                    INSERT INTO user_locations (
                        account_id, region_code, coordinates, accuracy_meters,
                        altitude, heading, speed, is_ghost_mode,
                        privacy_radius_meters, updated_at, version
                    ) VALUES (
                        $1, $2, ST_SetSRID(ST_MakePoint($3, $4), 4326)::geography,
                        $5, $6, $7, $8, $9, $10, $11, $12
                    )
                "#;

                    sqlx::query(insert_sql)
                        .bind(row.account_id)
                        .bind(&row.region_code)
                        .bind(row.lon)
                        .bind(row.lat)
                        .bind(row.accuracy_meters)
                        .bind(row.altitude)
                        .bind(row.heading)
                        .bind(row.speed)
                        .bind(row.is_ghost_mode)
                        .bind(row.privacy_radius_meters)
                        .bind(row.updated_at)
                        .bind(row.version) // Sera 1 si l'entité vient d'être créée
                        .execute(&mut *conn)
                        .await
                        .map_domain_infra("UserLocationInsert")?;
                } else {
                    // 4. Conflit de concurrence réel
                    return Err(shared_kernel::errors::DomainError::ConcurrencyConflict {
                        reason: format!("Location version mismatch for account {}", row.account_id)
                    });
                }
            }

            Ok(())
        })).await
    }

    async fn fetch(
        &self,
        account_id: &AccountId,
        region: &RegionCode,
    ) -> Result<Option<UserLocation>> {
        let sql = r#"
        SELECT
            account_id, region_code, ST_X(coordinates::geometry) as lon, ST_Y(coordinates::geometry) as lat,
            accuracy_meters, altitude, heading, speed, is_ghost_mode,
            privacy_radius_meters, updated_at,
            version,
            NULL as distance
        FROM user_locations
        WHERE account_id = $1 AND region_code = $2
    "#;

        let row = sqlx::query_as::<_, PostgresLocationRow>(sql)
            .bind(account_id.as_uuid())
            .bind(region.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_domain_infra("UserLocationFetch")?;

        row.map(|r| r.try_into()).transpose()
    }

    async fn fetch_nearby(
        &self,
        center: GeoPoint,
        region: RegionCode,
        radius_meters: f64,
        limit: i64,
    ) -> Result<Vec<(UserLocation, f64)>> {
        let sql = r#"
           SELECT
            account_id, region_code, ST_X(coordinates::geometry) as lon, ST_Y(coordinates::geometry) as lat,
            accuracy_meters, altitude, heading, speed, is_ghost_mode,
            privacy_radius_meters, updated_at,
            version,
            ST_Distance(coordinates, ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography) as distance
        FROM user_locations
            WHERE region_code = $3
              AND ST_DWithin(coordinates, ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography, $4)
              AND is_ghost_mode = FALSE
            ORDER BY distance ASC
            LIMIT $5
        "#;

        let rows = sqlx::query_as::<_, PostgresLocationRow>(sql)
            .bind(center.lon())
            .bind(center.lat())
            .bind(region.as_str())
            .bind(radius_meters)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_domain_infra("UserLocationNearby")?;

        rows.into_iter()
            .map(|r| {
                let dist = r.distance.unwrap_or(0.0);
                Ok((r.try_into()?, dist))
            })
            .collect()
    }

    async fn delete(&self, account_id: &AccountId, region: &RegionCode) -> Result<()> {
        let sql = "DELETE FROM user_locations WHERE account_id = $1 AND region_code = $2";

        sqlx::query(sql)
            .bind(account_id.as_uuid())
            .bind(region.as_str())
            .execute(&self.pool)
            .await
            .map_domain_infra("UserLocationDelete")?;

        Ok(())
    }
}
