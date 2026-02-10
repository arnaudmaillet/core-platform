// crates/profile/tests/infrastructure/composite_repository_it.rs

use std::sync::Arc;
use profile::domain::entities::Profile;
use profile::domain::repositories::{ProfileRepository, ProfileStatsRepository};
use profile::domain::value_objects::{DisplayName, Handle};
use profile::infrastructure::persistence_orchestrator::UnifiedProfileRepository;
use profile::infrastructure::postgres::repositories::PostgresIdentityRepository;
use profile::infrastructure::scylla::repositories::ScyllaProfileRepository;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::infrastructure::redis::repositories::RedisCacheRepository;
use shared_kernel::infrastructure::utils::{setup_full_infrastructure, InfrastructureTestContext};

struct CompositeTestContext {
    repo: Arc<UnifiedProfileRepository>,
    stats_repo: Arc<ScyllaProfileRepository>,
    infra : InfrastructureTestContext,
}

async fn setup_composite_test_context() -> CompositeTestContext {
    let infra = setup_full_infrastructure(
        &["./migrations/postgres"],
        &["./migrations/scylla"]
    ).await;

    // --- AJOUT DE SÉCURITÉ POUR SCYLLA ---
    // On vérifie que la session est vivante avant de passer aux tests
    infra.scylla_session
        .query_unpaged("SELECT now() FROM system.local", ())
        .await
        .expect("ScyllaDB session is broken or not ready");

    let cache_redis = {
        let repo = RedisCacheRepository::new(&infra.redis_url).await
            .expect("Failed to connect to Redis IT container");
        Arc::new(repo)
    };

    let identity_pg = Arc::new(PostgresIdentityRepository::new(infra.pg_pool.clone()));
    let stats_scylla = Arc::new(ScyllaProfileRepository::new(infra.scylla_session.clone()));

    let composite = Arc::new(UnifiedProfileRepository::new(
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
    let owner_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();

    // 1. Seed Identity (via Composite -> Postgres)
    let profile = Profile::builder(
        owner_id.clone(),
        region.clone(),
        DisplayName::from_raw("StatsUser"),
        Handle::try_new("stats_man").unwrap()
    ).build();

    let profile_id = profile.id().clone();
    ctx.repo.save_identity(&profile, None, None).await.unwrap();

    // 2. Seed Stats (via Scylla directement)
    ctx.stats_repo.save(&profile_id, &region, 10, 5, 0).await.unwrap();

    // 3. ASSEMBLE : Le composite doit fusionner Postgres et Scylla via le ProfileId
    let full_profile = ctx.repo.assemble_full_profile(&profile_id, &region).await.unwrap().expect("Should find profile");

    assert_eq!(full_profile.stats().follower_count(), 10);
    assert_eq!(full_profile.stats().following_count(), 5);
}

#[tokio::test]
async fn test_handle_change_invalidates_redis_index() {
    let ctx = setup_composite_test_context().await;
    let owner_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();
    let old_handle = Handle::try_new("old_name").unwrap();
    let new_handle = Handle::try_new("new_name").unwrap();

    // 1. Setup & Warmup Cache
    let profile = Profile::builder(
        owner_id,
        region.clone(),
        DisplayName::from_raw("Test"),
        old_handle.clone()
    ).build();

    ctx.repo.save_identity(&profile, None, None).await.unwrap();

    // On "chauffe" l'index Redis (crée une entrée profile:h:eu:old_name)
    ctx.repo.resolve_profile_from_handle(&old_handle, &region).await.unwrap();

    // 2. Changement de handle
    let original_profile = profile.clone();
    let mut updated_profile = profile;
    updated_profile.update_handle(&region, new_handle.clone()).unwrap();

    // L'orchestrateur UnifiedProfileRepository::save_identity doit détecter
    // le changement de handle et supprimer l'ancienne clé dans Redis.
    ctx.repo.save_identity(&updated_profile, Some(&original_profile), None).await.unwrap();

    // 3. Vérification de l'invalidation de l'ancien handle
    let old_res = ctx.repo.resolve_profile_from_handle(&old_handle, &region).await.unwrap();
    assert!(old_res.is_none(), "L'ancien index Redis devrait avoir été invalidé");

    // 4. Vérification de la nouvelle résolution
    let new_res = ctx.repo.resolve_profile_from_handle(&new_handle, &region).await.unwrap();
    assert!(new_res.is_some(), "Le nouveau handle doit être résolvable");
    assert_eq!(new_res.unwrap().id(), updated_profile.id());
}

#[tokio::test]
async fn test_profile_deletion_clears_all_repositories() {
    let ctx = setup_composite_test_context().await;
    let owner_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();
    let handle = Handle::try_new("to_delete").unwrap();

    let profile = Profile::builder(owner_id, region.clone(), DisplayName::from_raw("Bye"), handle.clone()).build();
    let pid = profile.id().clone();

    // On sauve partout
    ctx.repo.save_identity(&profile, None, None).await.unwrap();
    ctx.stats_repo.save(&pid, &region, 1, 1, 1).await.unwrap();
    ctx.repo.resolve_profile_from_handle(&handle, &region).await.unwrap(); // Warm cache

    // Act
    ctx.repo.delete_full_profile(&pid, &region).await.unwrap();

    // Assert
    let res_identity = ctx.repo.fetch_identity_only(&pid, &region).await.unwrap();
    let res_stats = ctx.stats_repo.fetch(&pid, &region).await.unwrap();
    let res_cache = ctx.repo.resolve_profile_from_handle(&handle, &region).await.unwrap();

    assert!(res_identity.is_none(), "Identity should be deleted from Postgres");
    assert!(res_stats.is_none(), "Stats should be deleted from ScyllaDB");
    assert!(res_cache.is_none(), "Index should be deleted from Redis");
}