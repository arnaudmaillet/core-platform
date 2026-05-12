// crates/profile/src/infrastructure/postgres/repositories/postgres_profile_tests.rs

use std::time::Duration;

use profile::repositories::ProfileRepository;
use profile::repositories_impl::PostgresProfileRepository;
use profile::value_objects::{DisplayName, Handle, ProfileId};
use tokio;

use profile::entities::Profile;

use shared_kernel::domain::Identifier;
use shared_kernel::domain::entities::Versioned;
use shared_kernel::domain::repositories::CacheRepository;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::errors::{DomainError, Result};
use shared_kernel::infrastructure::postgres::transactions::PostgresTransaction;
use shared_kernel::infrastructure::postgres::utils::PostgresTestContext;
use shared_kernel::infrastructure::redis::utils::RedisTestContext;

/// Helper pour instancier le repo et les infrastructures de test
async fn get_test_context() -> (
    PostgresProfileRepository,
    PostgresTestContext,
    RedisTestContext,
) {
    let pg_ctx = PostgresTestContext::builder()
        .with_migrations(&["./migrations/postgres"])
        .build()
        .await;

    let redis_ctx = RedisTestContext::builder().build().await;

    let repo = PostgresProfileRepository::new(pg_ctx.pool().clone(), redis_ctx.repository());

    (repo, pg_ctx, redis_ctx)
}

#[tokio::test]
async fn test_profile_full_lifecycle_and_atomicity() -> Result<()> {
    // --- Arrange ---
    let (repo, pg_ctx, _cache_ctx) = get_test_context().await;
    let account_id = AccountId::generate(RegionCode::default());
    let handle = Handle::try_new("alice_rocks")?;

    let mut profile = Profile::builder(account_id.clone(), handle)?.build()?;
    let profile_id = profile.profile_id().clone();
    let region = account_id.region().clone();

    // --- Act: Step 1 (Initial Save) ---
    repo.save(&mut profile, None).await?;

    // --- Assert: Step 2 (Verification Initial State) ---
    let found = repo
        .find_by_id(&profile_id, &region, None)
        .await?
        .expect("Profile should exist after initial save");

    assert_eq!(found.version(), 0); // Version initiale (Builder)

    // --- Act: Step 3 (Update with Domain Logic) ---
    let mut to_update = found.clone();
    to_update.update_display_name(DisplayName::try_new("Alice In Wonderland")?)?;
    // Ici, le domaine a incrémenté la version de 0 à 1

    repo.save(&mut to_update, None).await?;

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // --- Assert: Step 4 (Verification Updated State) ---
    let updated = repo
        .find_by_id(&profile_id, &region, None)
        .await?
        .expect("Profile should exist after update");

    assert_eq!(updated.display_name().as_str(), "Alice In Wonderland");
    assert_eq!(updated.version(), 1); // Version incrémentée par le domaine

    // --- Act: Step 5 (Transactional Delete) ---
    {
        let tx_sqlx = pg_ctx
            .pool()
            .begin()
            .await
            .map_err(|e| DomainError::Infrastructure(e.to_string()))?;
        let mut tx = PostgresTransaction::new(tx_sqlx);

        repo.delete(&profile_id, &region, Some(&mut tx)).await?;

        tx.into_inner()
            .commit()
            .await
            .map_err(|e| DomainError::Infrastructure(e.to_string()))?;
    }

    // --- Assert: Final State ---
    let deleted = repo.find_by_id(&profile_id, &region, None).await?;
    assert!(deleted.is_none(), "Profile should be null after deletion");

    Ok(())
}

#[tokio::test]
async fn test_profile_concurrency_protection_occ() -> Result<()> {
    let (repo, _pg_ctx, _cache_ctx) = get_test_context().await;
    let account_id = AccountId::generate(RegionCode::default());
    let mut profile =
        Profile::builder(account_id.clone(), Handle::try_new("ConcurrentUser")?)?.build()?;

    repo.save(&mut profile, None).await?;

    // Charger deux instances (v0)
    let mut client_a = repo
        .find_by_id(profile.profile_id(), account_id.region(), None)
        .await?
        .unwrap();
    let mut client_b = repo
        .find_by_id(profile.profile_id(), account_id.region(), None)
        .await?
        .unwrap();

    // Client A gagne
    client_a.update_display_name(DisplayName::try_new("Winner")?)?;
    repo.save(&mut client_a, None).await?; // Passe v1 en DB

    // Client B tente de save (v0 -> v1) mais la DB est déjà en v1
    client_b.update_display_name(DisplayName::try_new("Loser")?)?;
    let result = repo.save(&mut client_b, None).await;

    assert!(matches!(
        result,
        Err(DomainError::ConcurrencyConflict { .. })
    ));

    Ok(())
}

#[tokio::test]
async fn test_profile_cache_logic_integrity() -> Result<()> {
    let (repo, _pg_ctx, redis_ctx) = get_test_context().await;
    let cache = redis_ctx.repository();
    let account_id = AccountId::generate(RegionCode::default());
    let mut profile =
        Profile::builder(account_id.clone(), Handle::try_new("cache_test")?)?.build()?;

    let key = PostgresProfileRepository::cache_key(profile.profile_id(), account_id.region());

    // 1. Save & Initial Fetch
    repo.save(&mut profile, None).await?;
    // Ce find_by_id déclenche le tokio::spawn(set_obj)
    repo.find_by_id(profile.profile_id(), account_id.region(), None)
        .await?;

    // --- CORRECTION : Forcer le passage aux tâches de fond ---
    let mut filled = false;
    for _ in 0..30 {
        // On monte à 1.5s max
        tokio::task::yield_now().await; // On rend la main à l'exécuteur
        if cache.exists(&key).await? {
            filled = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(filled, "Cache should be filled (async spawn still pending)");

    // 2. Save (update) -> Doit invalider
    profile.update_display_name(DisplayName::try_new("Updated Name")?)?;
    repo.save(&mut profile, None).await?; // Déclenche tokio::spawn(delete)

    let mut invalidated = false;
    for _ in 0..30 {
        tokio::task::yield_now().await;
        if !cache.exists(&key).await? {
            invalidated = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(invalidated, "Cache must be invalidated after save");

    Ok(())
}

#[tokio::test]
async fn test_profile_rollback_works_properly() -> Result<()> {
    let (repo, pg_ctx, _cache_ctx) = get_test_context().await;
    let account_id = AccountId::generate(RegionCode::default());
    let mut profile = Profile::builder(account_id.clone(), Handle::try_new("Ghost")?)?.build()?;

    let tx_sqlx = pg_ctx
        .pool()
        .begin()
        .await
        .map_err(|e| DomainError::Infrastructure(e.to_string()))?;
    let mut tx = PostgresTransaction::new(tx_sqlx);

    repo.save(&mut profile, Some(&mut tx)).await?;
    tx.into_inner()
        .rollback()
        .await
        .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

    let found = repo
        .find_by_id(profile.profile_id(), account_id.region(), None)
        .await?;
    assert!(found.is_none(), "Profile should not exist after rollback");

    Ok(())
}

#[tokio::test]
async fn test_find_all_by_account_id() -> Result<()> {
    let (repo, _pg_ctx, _cache_ctx) = get_test_context().await;
    let account_id = AccountId::generate(RegionCode::default());

    // Utilise "profile_1" (minuscules) pour correspondre à la sortie attendue
    let mut p1 = Profile::builder(account_id.clone(), Handle::try_new("profile_1")?)?.build()?;
    let mut p2 = Profile::builder(account_id.clone(), Handle::try_new("profile_2")?)?.build()?;

    repo.save(&mut p1, None).await?;
    repo.save(&mut p2, None).await?;

    let profiles = repo.find_all_by_account_id(&account_id, None).await?;

    assert_eq!(profiles.len(), 2);
    // On compare avec "profile_2" (en minuscules)
    assert_eq!(profiles[0].handle().as_str(), "profile_2");

    Ok(())
}

#[tokio::test]
async fn test_cache_hit_proven_by_db_sabotage() -> Result<()> {
    let (repo, pg_ctx, _cache_ctx) = get_test_context().await;
    let account_id = AccountId::generate(RegionCode::default());
    let mut profile = Profile::builder(account_id.clone(), Handle::try_new("alice")?)?.build()?;

    repo.save(&mut profile, None).await?;
    repo.find_by_id(profile.profile_id(), account_id.region(), None)
        .await?;

    // --- CORRECTION : Attendre que le cache soit peuplé ---
    let key = PostgresProfileRepository::cache_key(profile.profile_id(), account_id.region());
    let mut cached = false;
    for _ in 0..30 {
        tokio::task::yield_now().await;
        if _cache_ctx.repository().exists(&key).await? {
            cached = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(cached, "Cache setup failed before sabotage");

    // 3. Sabotage DB
    sqlx::query("DELETE FROM user_profiles WHERE profile_id = $1")
        .bind(profile.profile_id().as_uuid())
        .execute(&pg_ctx.pool())
        .await
        .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

    // 4. Lecture (Bypass DB car déjà en cache)
    let found_from_cache = repo
        .find_by_id(profile.profile_id(), account_id.region(), None)
        .await?;

    assert!(
        found_from_cache.is_some(),
        "Should return object from cache even if DB is empty"
    );

    Ok(())
}

#[tokio::test]
async fn test_profile_rollback_integrity() -> Result<()> {
    let (repo, pg_ctx, _cache_ctx) = get_test_context().await;
    let account_id = AccountId::generate(RegionCode::default());
    let mut profile = Profile::builder(account_id.clone(), Handle::try_new("ghost")?)?.build()?;
    let profile_id = profile.profile_id().clone();

    // 1. On ouvre une transaction et on save
    let tx_sqlx = pg_ctx
        .pool()
        .begin()
        .await
        .map_err(|e| DomainError::Infrastructure(e.to_string()))?;
    let mut tx = PostgresTransaction::new(tx_sqlx);
    repo.save(&mut profile, Some(&mut tx)).await?;

    // 2. ROLLBACK
    tx.into_inner()
        .rollback()
        .await
        .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

    // 3. Le profil ne doit pas exister
    let found = repo
        .find_by_id(&profile_id, account_id.region(), None)
        .await?;
    assert!(found.is_none(), "Profile should not exist after rollback");

    Ok(())
}

#[tokio::test]
async fn test_profile_partial_data_resilience() -> Result<()> {
    let (repo, pg_ctx, _cache_ctx) = get_test_context().await;
    let profile_id = uuid::Uuid::new_v4();
    let account_id = uuid::Uuid::new_v4();

    // INSERTION MANUELLE avec le strict minimum (beaucoup de NULLs)
    sqlx::query(
        r#"INSERT INTO user_profiles (profile_id, account_id, region_code, display_name, handle, is_private, version, created_at, updated_at)
           VALUES ($1, $2, 'EU', 'Minimalist', 'mini', false, 1, NOW(), NOW())"#
    )
    .bind(profile_id)
    .bind(account_id)
    .execute(&pg_ctx.pool())
    .await
    .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

    // On tente de charger cet agrégat "incomplet"
    let result = repo
        .find_by_id(
            &ProfileId::from(profile_id),
            &RegionCode::try_new("EU")?,
            None,
        )
        .await?;

    assert!(result.is_some());
    let p = result.unwrap();
    assert_eq!(p.display_name().as_str(), "Minimalist");
    assert!(p.avatar().is_none());
    assert!(p.bio().is_none());

    Ok(())
}
