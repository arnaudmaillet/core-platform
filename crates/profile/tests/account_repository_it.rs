// crates/profile/src/infrastructure/postgres/repositories/postgres_profile_tests.rs

use profile::repositories::ProfileRepository;
use profile::repositories_impl::PostgresProfileRepository;
use profile::types::{DisplayName, Handle};
use tokio;

use profile::entities::Profile;

use shared_kernel::core::{Error, ErrorCode, Identifier, Result, Versioned};
use shared_kernel::postgres::PostgresTransaction;
use shared_kernel::test_utils::{PostgresTestContext, RedisTestContext};
use shared_kernel::types::{AccountId, ProfileId, Region};

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

    let repo = PostgresProfileRepository::new(pg_ctx.pool().clone());

    (repo, pg_ctx, redis_ctx)
}

#[tokio::test]
async fn test_profile_full_lifecycle_and_atomicity() -> Result<()> {
    // --- Arrange ---
    let (repo, pg_ctx, _cache_ctx) = get_test_context().await;
    let account_id = AccountId::generate(Region::default());
    let handle = Handle::try_new("alice_rocks")?;

    let mut profile = Profile::builder(account_id, handle)?.build()?;
    let profile_id = profile.profile_id();
    let region = account_id.region();

    // --- Act: Step 1 (Initial Save) ---
    repo.save(&mut profile, None).await?;

    // --- Assert: Step 2 (Verification Initial State) ---
    let found = repo
        .find_by_id(profile_id, region, None)
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
        .find_by_id(profile_id, region, None)
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
            .map_err(|e| Error::internal(e.to_string()))?;
        let mut tx = PostgresTransaction::new(tx_sqlx);

        repo.delete(profile_id, region, Some(&mut tx)).await?;

        tx.into_inner()
            .commit()
            .await
            .map_err(|e| Error::internal(e.to_string()))?;
    }

    // --- Assert: Final State ---
    let deleted = repo.find_by_id(profile_id, region, None).await?;
    assert!(deleted.is_none(), "Profile should be null after deletion");

    Ok(())
}

#[tokio::test]
async fn test_profile_concurrency_protection_occ() -> Result<()> {
    let (repo, _pg_ctx, _cache_ctx) = get_test_context().await;
    let account_id = AccountId::generate(Region::default());
    let mut profile = Profile::builder(account_id, Handle::try_new("ConcurrentUser")?)?.build()?;

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
        Err(e) if e.code == ErrorCode::ConcurrencyConflict
    ));
    Ok(())
}

#[tokio::test]
async fn test_profile_rollback_works_properly() -> Result<()> {
    let (repo, pg_ctx, _cache_ctx) = get_test_context().await;
    let account_id = AccountId::generate(Region::default());
    let mut profile = Profile::builder(account_id, Handle::try_new("Ghost")?)?.build()?;

    let tx_sqlx = pg_ctx
        .pool()
        .begin()
        .await
        .map_err(|e| Error::internal(e.to_string()))?;
    let mut tx = PostgresTransaction::new(tx_sqlx);

    repo.save(&mut profile, Some(&mut tx)).await?;
    tx.into_inner()
        .rollback()
        .await
        .map_err(|e| Error::internal(e.to_string()))?;

    let found = repo
        .find_by_id(profile.profile_id(), account_id.region(), None)
        .await?;
    assert!(found.is_none(), "Profile should not exist after rollback");

    Ok(())
}

#[tokio::test]
async fn test_find_all_by_account_id() -> Result<()> {
    let (repo, _pg_ctx, _cache_ctx) = get_test_context().await;
    let account_id = AccountId::generate(Region::default());

    // Utilise "profile_1" (minuscules) pour correspondre à la sortie attendue
    let mut p1 = Profile::builder(account_id, Handle::try_new("profile_1")?)?.build()?;
    let mut p2 = Profile::builder(account_id, Handle::try_new("profile_2")?)?.build()?;

    repo.save(&mut p1, None).await?;
    repo.save(&mut p2, None).await?;

    let profiles = repo.find_all_by_account_id(account_id, None).await?;

    assert_eq!(profiles.len(), 2);
    // On compare avec "profile_2" (en minuscules)
    assert_eq!(profiles[0].handle().as_str(), "profile_2");

    Ok(())
}

#[tokio::test]
async fn test_profile_rollback_integrity() -> Result<()> {
    let (repo, pg_ctx, _cache_ctx) = get_test_context().await;
    let account_id = AccountId::generate(Region::default());
    let mut profile = Profile::builder(account_id, Handle::try_new("ghost")?)?.build()?;
    let profile_id = profile.profile_id();

    // 1. On ouvre une transaction et on save
    let tx_sqlx = pg_ctx
        .pool()
        .begin()
        .await
        .map_err(|e| Error::internal(e.to_string()))?;
    let mut tx = PostgresTransaction::new(tx_sqlx);
    repo.save(&mut profile, Some(&mut tx)).await?;

    // 2. ROLLBACK
    tx.into_inner()
        .rollback()
        .await
        .map_err(|e| Error::internal(e.to_string()))?;

    // 3. Le profil ne doit pas exister
    let found = repo
        .find_by_id(profile_id, account_id.region(), None)
        .await?;
    assert!(found.is_none(), "Profile should not exist after rollback");

    Ok(())
}

#[tokio::test]
async fn test_profile_partial_data_resilience() -> Result<()> {
    let (repo, pg_ctx, _cache_ctx) = get_test_context().await;
    let region = Region::try_new("EU")?;

    let domain_profile_id = ProfileId::generate(region.clone());
    let domain_account_id = AccountId::generate(region.clone());

    let profile_uuid = domain_profile_id.as_uuid();
    let account_uuid = domain_account_id.uuid();

    sqlx::query(
        r#"INSERT INTO user_profiles (profile_id, account_id, region, display_name, handle, is_private, version, created_at, updated_at)
           VALUES ($1, $2, 'EU', 'Minimalist', 'mini', false, 1, NOW(), NOW())"#
    )
    .bind(profile_uuid)
    .bind(account_uuid)
    .execute(&pg_ctx.pool())
    .await
    .map_err(|e| Error::database(e.to_string()))?;

    let fetch_res = repo.find_by_id(domain_profile_id, region, None).await;

    let result = match fetch_res {
        Ok(opt) => opt,
        Err(e) => {
            println!("\n💥 [DEBUG CRASH] find_by_id a renvoyé une ERREUR !");
            println!("👉 Code d'erreur : {:?}", e.code);
            println!("👉 Message complet : {}", e.message);
            panic!(
                "Le repository a échoué à charger les données partielles : {}",
                e.message
            );
        }
    };

    assert!(
        result.is_some(),
        "Le profil aurait dû être trouvé (None renvoyé)"
    );
    let p = result.unwrap();
    assert_eq!(p.display_name().as_str(), "Minimalist");
    assert!(p.avatar().is_none());
    assert!(p.bio().is_none());

    Ok(())
}
