// crates/geo_discovery/src/infrastructure/repositories/scylla_map_persistence_repository.rs

use async_trait::async_trait;
use infra_scylla::scylla::errors::PrepareError;
use infra_scylla::scylla::{
    client::session::Session, statement::prepared::PreparedStatement, value::CqlTimestamp,
};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::PostId;
use std::sync::Arc;
use std::time::Duration;

use crate::entities::ActiveMapPost;
use crate::mappers::CqlMapPostRow;
use crate::repositories::MapPersistenceRepository;
use crate::types::{BucketHour, H3Tile, TileResolution};

macro_rules! insert_tile_cql {
    () => {
        "INSERT INTO geo_discovery.active_posts_by_tile \
         (tile_resolution, tile_id, bucket_hour, post_id, latitude, longitude, post_type, thumbnail_url, created_at, expires_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
         USING TTL ?"
    };
}

macro_rules! find_by_tile_cql {
    () => {
        "SELECT tile_resolution, tile_id, bucket_hour, post_id, latitude, longitude, post_type, thumbnail_url, created_at, expires_at \
         FROM geo_discovery.active_posts_by_tile \
         WHERE tile_resolution = ? AND tile_id = ? AND bucket_hour = ?"
    };
}

macro_rules! delete_tile_cql {
    () => {
        "DELETE FROM geo_discovery.active_posts_by_tile \
         WHERE tile_resolution = ? AND tile_id = ? AND bucket_hour = ? AND post_id = ?"
    };
}

pub struct ScyllaMapPersistenceRepository {
    session: Arc<Session>,
    insert_tile_stmt: PreparedStatement,
    find_by_tile_stmt: PreparedStatement,
    delete_tile_stmt: PreparedStatement,
}

impl ScyllaMapPersistenceRepository {
    pub async fn new(session: Arc<Session>) -> std::result::Result<Self, PrepareError> {
        let insert_tile_stmt = session.prepare(insert_tile_cql!()).await?;
        let find_by_tile_stmt = session.prepare(find_by_tile_cql!()).await?;
        let delete_tile_stmt = session.prepare(delete_tile_cql!()).await?;

        Ok(Self {
            session,
            insert_tile_stmt,
            find_by_tile_stmt,
            delete_tile_stmt,
        })
    }
}

#[async_trait]
impl MapPersistenceRepository for ScyllaMapPersistenceRepository {
    async fn save(&self, post: &ActiveMapPost, ttl: Duration) -> Result<()> {
        let ttl_seconds = ttl.as_secs() as i32;
        let bucket_cql = CqlTimestamp(post.bucket_hour().value());
        let created_at_cql = CqlTimestamp(post.created_at().timestamp_millis());
        let expires_at_cql = CqlTimestamp(post.expires_at().timestamp_millis());

        let post_type_str = post.post_type().to_string();
        let thumbnail_cql = post.thumbnail_url().map(|url| url.to_string());

        let params = (
            post.resolution().value(),
            post.tile_id().value().to_string(),
            bucket_cql,
            post.post_id().uuid(),
            post.location().lat(),
            post.location().lon(),
            post_type_str,
            thumbnail_cql,
            created_at_cql,
            expires_at_cql,
            ttl_seconds,
        );

        self.session
            .execute_unpaged(&self.insert_tile_stmt, &params)
            .await
            .map_err(|e| Error::database(format!("Scylla map save failed: {}", e)))?;

        Ok(())
    }

    async fn find_by_tile(
        &self,
        resolution: TileResolution,
        tile_id: &H3Tile,
        bucket: BucketHour,
    ) -> Result<Vec<ActiveMapPost>> {
        let bucket_cql = CqlTimestamp(bucket.value());

        let query_res = self
            .session
            .execute_unpaged(
                &self.find_by_tile_stmt,
                (resolution.value(), tile_id.value().to_string(), bucket_cql),
            )
            .await
            .map_err(|e| Error::database(format!("Scylla find_by_tile failed: {}", e)))?;

        let rows_result = query_res
            .into_rows_result()
            .map_err(|e| Error::database(format!("Invalid geo rows format: {}", e)))?;

        let rows_iter = rows_result
            .rows::<CqlMapPostRow>()
            .map_err(|e| Error::database(format!("Geo row iterator failure: {}", e)))?;

        let mut posts = Vec::new();
        for row_res in rows_iter {
            let cql_row =
                row_res.map_err(|e| Error::database(format!("Geo row parsing failed: {}", e)))?;
            posts.push(ActiveMapPost::try_from(cql_row)?);
        }

        Ok(posts)
    }

    async fn delete(
        &self,
        resolution: TileResolution,
        tile_id: &H3Tile,
        bucket: BucketHour,
        post_id: &PostId,
    ) -> Result<()> {
        let bucket_cql = CqlTimestamp(bucket.value());

        self.session
            .execute_unpaged(
                &self.delete_tile_stmt,
                (
                    resolution.value(),
                    tile_id.value().to_string(),
                    bucket_cql,
                    post_id.uuid(),
                ),
            )
            .await
            .map_err(|e| Error::database(format!("Scylla map delete failed: {}", e)))?;

        Ok(())
    }
}
