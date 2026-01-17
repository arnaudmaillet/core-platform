// crates/profile/src/infrastructure/repositories/postgres_location_repository/mod.rs

use async_trait::async_trait;
use sqlx::PgPool;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{GeoPoint, RegionCode, AccountId};
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::postgres::SqlxErrorExt;
use crate::domain::entities::UserLocation;
use crate::domain::repositories::LocationRepository;
use crate::infrastructure::postgres::rows::PostgresLocationRow;

pub struct PostgresLocationRepository {
    pool: PgPool,
}

#[async_trait]
impl LocationRepository for PostgresLocationRepository {
    async fn save(&self, loc: &UserLocation, tx: Option<&mut dyn Transaction>) -> Result<()> {
        let pool = self.pool.clone();
        let l = loc.clone();

        <dyn Transaction>::execute_on(&pool, tx, |conn| Box::pin(async move {
            let sql = r#"
                INSERT INTO user_locations (
                    account_id, region_code, coordinates, accuracy_meters,
                    altitude, heading, speed, is_ghost_mode,
                    privacy_radius_meters, updated_at
                )
                VALUES (
                    $1, $2,
                    ST_SetSRID(ST_MakePoint($3, $4), 4326)::geography,
                    $5, $6, $7, $8, $9, $10, $11
                )
                ON CONFLICT (account_id, region_code) DO UPDATE SET
                    coordinates = EXCLUDED.coordinates,
                    accuracy_meters = EXCLUDED.accuracy_meters,
                    altitude = EXCLUDED.altitude,
                    heading = EXCLUDED.heading,
                    speed = EXCLUDED.speed,
                    is_ghost_mode = EXCLUDED.is_ghost_mode,
                    privacy_radius_meters = EXCLUDED.privacy_radius_meters,
                    updated_at = EXCLUDED.updated_at
            "#;

            sqlx::query(sql)
                .bind(l.account_id.as_uuid())
                .bind(l.region_code.as_str())
                .bind(l.coordinates.lon())
                .bind(l.coordinates.lat())
                .bind(l.metrics.as_ref().map(|m| m.accuracy().value()))
                .bind(l.metrics.as_ref().and_then(|m| m.altitude().map(|a| a.value())))
                .bind(l.movement.as_ref().map(|m| m.heading().value()))
                .bind(l.movement.as_ref().map(|m| m.speed().value()))
                .bind(l.is_ghost_mode)
                .bind(l.privacy_radius_meters)
                .bind(l.updated_at)
                .execute(conn)
                .await
                .map_domain_infra("UserLocationSave")?;

            Ok(())
        })).await
    }

    async fn find_by_id(&self, account_id: &AccountId, region: &RegionCode) -> Result<Option<UserLocation>> {
        let sql = r#"
            SELECT
                account_id, region_code, ST_X(coordinates::geometry) as lon, ST_Y(coordinates::geometry) as lat,
                accuracy_meters, altitude, heading, speed, is_ghost_mode,
                privacy_radius_meters, updated_at, NULL as distance
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

    async fn find_nearby(
        &self,
        center: GeoPoint,
        region: RegionCode,
        radius_meters: f64,
        limit: i64
    ) -> Result<Vec<(UserLocation, f64)>> {
        let sql = r#"
            SELECT
                account_id, region_code, ST_X(coordinates::geometry) as lon, ST_Y(coordinates::geometry) as lat,
                accuracy_meters, altitude, heading, speed, is_ghost_mode,
                privacy_radius_meters, updated_at,
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

        let mut results = Vec::new();
        for r in rows {
            let distance = r.distance.unwrap_or(0.0);
            let loc: UserLocation = r.try_into()?;
            results.push((loc, distance));
        }

        Ok(results)
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