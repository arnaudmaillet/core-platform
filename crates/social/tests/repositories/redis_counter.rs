
use chrono::Utc;
use infra_fred::fred::interfaces::SetsInterface;
use infra_test::RedisTestContext;
use shared_kernel::core::{ErrorCode, Result};
use shared_kernel::types::{Counter, ProfileId};
use social::entities::ProfileCounters;
use social::redis::RedisCounterRepository;
use social::repositories::CounterRepository;

/// Helper pour instancier le dépôt connecté au cluster Redis de test du shared-kernel
async fn get_test_context() -> (RedisCounterRepository, RedisTestContext) {
    // Utilisation native de ton builder du shared_kernel
    let test_ctx = RedisTestContext::builder()
        .with_image("7.2-alpine")
        .build()
        .await;

    // Extraction du pool fred via le RedisCacheRepository exposé par le contexte
    let pool = test_ctx.repository().pool().clone();
    let repo = RedisCounterRepository::new(pool);

    (repo, test_ctx)
}

#[tokio::test]
async fn test_redis_get_counters_should_return_not_found_when_empty() -> Result<()> {
    // --- Arrange ---
    let (repo, _test_ctx) = get_test_context().await;
    let random_id = ProfileId::generate();

    // --- Act ---
    let res = repo.get_counters(random_id).await;

    // --- Assert ---
    // Ton RedisCounterRepository renvoie une Error::NotFound si le Hash est vide
    assert!(res.is_err());
    let error = res.unwrap_err();
    assert_eq!(
        error.code,
        ErrorCode::NotFound,
        "L'erreur attendue était un code NotFound, obtenu: {:?}",
        error
    );
    Ok(())
}

#[tokio::test]
async fn test_redis_counter_increment_and_dirty_set_lifecycle() -> Result<()> {
    // --- Arrange ---
    let (repo, test_ctx) = get_test_context().await;

    let follower_id = ProfileId::generate();
    let following_id = ProfileId::generate();
    let pool = test_ctx.repository().pool().clone();

    // --- Act: Étape 1 - Incrémentation atomique des Hashes ---
    repo.increment_counters(follower_id, following_id).await?;

    // --- Assert: Étape 2 - Validation des compteurs dans Redis ---
    let follower_cache = repo.get_counters(follower_id).await?;
    assert_eq!(follower_cache.following_count().value(), 1);
    assert_eq!(follower_cache.followers_count().value(), 0);

    let following_cache = repo.get_counters(following_id).await?;
    assert_eq!(following_cache.followers_count().value(), 1);
    assert_eq!(following_cache.following_count().value(), 0);

    // --- Assert: Étape 3 - Validation du marquage "Dirty" pour le worker de réconciliation ---
    let is_follower_dirty: bool = pool
        .sismember("profiles:dirty", follower_id.to_string())
        .await
        .unwrap();
    let is_following_dirty: bool = pool
        .sismember("profiles:dirty", following_id.to_string())
        .await
        .unwrap();

    assert!(
        is_follower_dirty,
        "Le follower doit être marqué dans le Set profiles:dirty"
    );
    assert!(
        is_following_dirty,
        "La cible doit être marquée dans le Set profiles:dirty"
    );

    // --- Act: Étape 4 - Décrémentation ---
    repo.decrement_counters(follower_id, following_id).await?;

    // --- Assert: Étape 5 - Les compteurs descendent à 0 ---
    let follower_cache_after = repo.get_counters(follower_id).await?;
    assert_eq!(follower_cache_after.following_count().value(), 0);

    let following_cache_after = repo.get_counters(following_id).await?;
    assert_eq!(following_cache_after.followers_count().value(), 0);

    Ok(())
}

#[tokio::test]
async fn test_redis_save_should_overwrite_entire_hash() -> Result<()> {
    // --- Arrange ---
    let (repo, _test_ctx) = get_test_context().await;
    let profile_id = ProfileId::generate();

    // Simulation d'un snapshot à écraser/synchroniser (ex: flush depuis Scylla)
    let counters_snapshot = ProfileCounters::restore(
        profile_id,
        Counter::from_raw(42),
        Counter::from_raw(84),
        1,
        Utc::now(),
        Utc::now(),
    );

    // --- Act ---
    repo.save(&counters_snapshot).await?;

    // --- Assert ---
    let cache = repo.get_counters(profile_id).await?;
    assert_eq!(cache.followers_count().value(), 42);
    assert_eq!(cache.following_count().value(), 84);

    Ok(())
}
