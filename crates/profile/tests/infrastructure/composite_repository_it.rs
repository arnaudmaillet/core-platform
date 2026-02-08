// crates/profile/tests/infrastructure/composite_repository_it.rs

use std::sync::Arc;
use profile::domain::entities::Profile;
use profile::domain::repositories::{ProfileRepository, ProfileStatsRepository};
use profile::domain::value_objects::DisplayName;
use profile::infrastructure::postgres::repositories::PostgresProfileRepository;
use profile::infrastructure::scylla::repositories::ScyllaProfileRepository;
use profile::infrastructure::repositories::CompositeProfileRepository;
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
use shared_kernel::infrastructure::redis::repositories::RedisCacheRepository;
use shared_kernel::infrastructure::utils::{setup_full_infrastructure, InfrastructureTestContext};

struct CompositeTestContext {
    repo: Arc<CompositeProfileRepository>,
    stats_repo: Arc<ScyllaProfileRepository>,
    infra : InfrastructureTestContext,
}

async fn setup_composite_test_context() -> CompositeTestContext {
    let infra = setup_full_infrastructure(
        &["./migrations/postgres"],
        &["./migrations/scylla"]
    ).await;

    // Utilisation d'un bloc pour forcer l'initialisation propre
    let cache_redis = {
        let repo = RedisCacheRepository::new(&infra.redis_url).await
            .expect("Failed to connect to Redis IT container");
        Arc::new(repo)
    };

    let identity_pg = Arc::new(PostgresProfileRepository::new(infra.pg_pool.clone()));
    let stats_scylla = Arc::new(ScyllaProfileRepository::new(infra.scylla_session.clone()));

    let composite = Arc::new(CompositeProfileRepository::new(
        identity_pg,
        stats_scylla.clone(),
        cache_redis.clone(),
    ));

    CompositeTestContext {
        repo: composite,
        stats_repo: stats_scylla,
        infra,
    }
}

#[tokio::test]
async fn test_stats_merging_from_scylla_to_composite() {
    let ctx = setup_composite_test_context().await;
    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();

    // 1. Seed Identity (via Composite -> Postgres)
    let profile = Profile::builder(
        account_id.clone(),
        region.clone(),
        DisplayName::from_raw("StatsUser"),
        Username::try_new("stats_man").unwrap()
    ).build();
    ctx.repo.save_identity(&profile, None, None).await.unwrap();

    // 2. Seed Stats (via Scylla directement car plus d'increment_stats dans le Composite)
    // On simule ce qu'un Worker Kafka ferait en arrière-plan
    ctx.stats_repo.save(&account_id, &region, 10, 5, 0).await.unwrap();

    // 3. ASSEMBLE : Le composite doit fusionner Postgres et Scylla
    let full_profile = ctx.repo.assemble_full_profile(&account_id, &region).await.unwrap().unwrap();

    assert_eq!(full_profile.stats().follower_count(), 10);
    assert_eq!(full_profile.stats().following_count(), 5);

    // Donne une chance aux tâches de fond de se terminer avant de couper les containers
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
}

#[tokio::test]
async fn test_username_change_invalidates_redis_index() {
    let ctx = setup_composite_test_context().await;
    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();
    let old_un = Username::try_new("old_name").unwrap();
    let new_un = Username::try_new("new_name").unwrap();

    // 1. Setup & Warmup Cache
    let profile = Profile::builder(account_id, region.clone(), DisplayName::from_raw("Test"), old_un.clone()).build();
    ctx.repo.save_identity(&profile, None, None).await.unwrap();

    // On "chauffe" l'index Redis
    ctx.repo.resolve_profile_from_username(&old_un, &region).await.unwrap();

    // 2. Changement de pseudo
    let original_profile = profile.clone();
    let mut updated_profile = profile;
    updated_profile.update_username(&region, new_un.clone()).unwrap();

    // C'est ici que Composite::save_identity doit faire son job d'invalidation
    ctx.repo.save_identity(&updated_profile, Some(&original_profile), None).await.unwrap();

    // 3. Vérification de l'invalidation
    let old_res = ctx.repo.resolve_profile_from_username(&old_un, &region).await.unwrap();
    assert!(old_res.is_none(), "L'ancien index Redis devrait avoir été supprimé car le pseudo a changé");

    // 4. Vérification de la nouvelle résolution (Cache Miss -> Re-remplissage)
    let new_res = ctx.repo.resolve_profile_from_username(&new_un, &region).await.unwrap();
    assert!(new_res.is_some(), "Le nouveau pseudo doit être résolvable");
    assert_eq!(new_res.unwrap().account_id(), updated_profile.account_id());
    let _ = ctx.repo.resolve_profile_from_username(&new_un, &region).await;
}