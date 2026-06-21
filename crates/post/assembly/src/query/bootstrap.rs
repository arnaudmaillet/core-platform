use crate::{PostAssemblyQueryService, PostQueryContainer};
use infra_fred::RedisCacheRepository;
use infra_scylla::scylla::client;
use post_older::{CachedPostReadRepository, ScyllaPostReadRepository};
use post_profile::{CachedProfileReadRepository, ScyllaProfileReadProjection};
use std::sync::Arc;

pub struct PostQueryAssembly;

impl PostQueryAssembly {
    pub async fn bootstrap(
        session: Arc<client::session::Session>,
        cache_repo: RedisCacheRepository,
        keyspace_name: String,
    ) -> Result<PostQueryContainer, shared_kernel::core::Error> {
        let scylla_post_read_repo =
            ScyllaPostReadRepository::new(session.clone(), keyspace_name.clone()).await?;
        let post_read_repo = Arc::new(CachedPostReadRepository::new(
            scylla_post_read_repo,
            cache_repo.clone(),
        ));

        let scylla_profile_read_projection =
            ScyllaProfileReadProjection::new(session.clone(), keyspace_name.clone()).await?;
        let profile_reader = Arc::new(CachedProfileReadRepository::new(
            scylla_profile_read_projection,
            cache_repo,
        ));

        let query_service = PostAssemblyQueryService {
            post_reader: post_read_repo,
            profile_reader,
        };

        Ok(PostQueryContainer { query_service })
    }
}
