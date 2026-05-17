use account::db::PostgresAccountRepository;
use account::entities::Account;
use account::repositories::AccountRepository;
use account::types::{AccountRole, AccountState, RegistrationIdentifier};
use shared_kernel::cache::CacheRepository;
use shared_kernel::postgres::PostgresTransaction;
use shared_kernel::test_utils::{PostgresTestContext, RedisTestContext};
use std::time::Duration;
use tokio;

use shared_kernel::core::{Error, Identifier, Result, Versioned};
use shared_kernel::types::{AccountId, AuditReason, Email, RegionCode, SubId};

/// Helper pour instancier le repo et les infrastructures de test (Postgres + Redis)
async fn get_test_context() -> (
    PostgresAccountRepository,
    PostgresTestContext,
    RedisTestContext,
) {
    let pg_ctx = PostgresTestContext::builder()
        .with_migrations(&["./migrations/postgres"])
        .build()
        .await;

    let redis_ctx = RedisTestContext::builder().build().await;

    let repo = PostgresAccountRepository::new(pg_ctx.pool().clone(), redis_ctx.repository());

    (repo, pg_ctx, redis_ctx)
}

#[tokio::test]
async fn test_account_full_lifecycle_and_atomicity() -> Result<()> {
    let (repo, pg_ctx, _) = get_test_context().await;
    let account_id = AccountId::generate(RegionCode::default());
    let email = Email::try_new("full@lifecycle.com")?;

    let account = Account::builder(
        account_id,
        RegistrationIdentifier::try_from_email(email.to_string())?,
    )
    .build()?;

    // --- 1. CREATE (Scope isolé) ---
    {
        let tx_sqlx = pg_ctx
            .pool()
            .begin()
            .await
            .map_err(|e| Error::internal(e.to_string()))?;
        let mut tx = PostgresTransaction::new(tx_sqlx);

        repo.create(&account, &mut tx).await?;

        tx.into_inner()
            .commit()
            .await
            .map_err(|e| Error::internal(e.to_string()))?;
    }

    // --- 2. FETCH ---
    let found = repo
        .find_by_id(account_id, None)
        .await?
        .expect("Account should exist");

    assert_eq!(found.identity().account_id(), account_id);
    assert_eq!(found.version(), 0);

    // --- 3. UPDATE ---
    let mut to_update = found.clone();
    to_update.deactivate(Some(AuditReason::system("deactivate test")))?;
    let _ = to_update.change_role(AccountRole::ADMIN, AuditReason::system("Change Governance"));

    repo.save(&mut to_update, None).await?;

    // --- 4. VERIFY UPDATE ---
    let updated = repo.find_by_id(account_id, None).await?.unwrap();
    assert_eq!(*updated.identity().state(), AccountState::DEACTIVATED);
    assert_eq!(updated.version(), 1);

    // --- 5. DELETE (Scope isolé) ---
    {
        let mut tx_del = PostgresTransaction::new(pg_ctx.pool().begin().await.unwrap());
        repo.delete(account_id, &mut tx_del).await?;
        tx_del.into_inner().commit().await.unwrap();
    }

    let deleted = repo.find_by_id(account_id, None).await?;
    assert!(deleted.is_none());

    Ok(())
}

#[tokio::test]
async fn test_concurrency_protection_occ() -> Result<()> {
    let (repo, pg_ctx, _) = get_test_context().await;
    let account_id = AccountId::generate(RegionCode::default());
    let account = Account::builder(
        account_id,
        RegistrationIdentifier::try_from_email("occ@test.com")?,
    )
    .build()?;

    let mut tx = PostgresTransaction::new(pg_ctx.pool().begin().await.unwrap());
    repo.create(&account, &mut tx).await?;
    tx.into_inner().commit().await.unwrap();

    let mut client_a = repo.find_by_id(account.account_id(), None).await?.unwrap();
    let mut client_b = repo.find_by_id(account.account_id(), None).await?.unwrap();

    client_a.activate()?;
    repo.save(&mut client_a, None).await?; // SQL: WHERE version = 0. OK.
    client_b.deactivate(None)?;
    let result = repo.save(&mut client_b, None).await;

    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn test_cache_logic_integrity() -> Result<()> {
    let (repo, pg_ctx, redis_ctx) = get_test_context().await;
    let cache = redis_ctx.repository();
    let account_id = AccountId::generate(RegionCode::default());

    // Utiliser la même logique de clé que le Repo ---
    // Tu peux soit appeler PostgresAccountRepository::cache_key(account_id)
    // Soit reproduire exactement le format :
    let cache_key = format!(
        "account:aggregate:{}:{}",
        account_id.region().as_str(),
        account_id.uuid()
    );

    let account = Account::builder(
        account_id,
        RegistrationIdentifier::try_from_email("cache@test.com")?,
    )
    .build()?;

    // 1. Create + find_by_id
    let mut tx = PostgresTransaction::new(pg_ctx.pool().begin().await.unwrap());
    repo.create(&account, &mut tx).await?;
    tx.into_inner().commit().await.unwrap();

    let _ = repo.find_by_id(account_id, None).await?;

    // --- AJUSTEMENT 2 : Assertion robuste avec retries ---
    let mut success = false;
    for _ in 0..10 {
        if cache.exists(&cache_key).await? {
            success = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        success,
        "Cache should be filled (checked with key: {})",
        cache_key
    );

    // 3. Save -> Invalide le cache
    let mut to_update = account.clone();
    to_update.activate()?;
    repo.save(&mut to_update, None).await?;

    // Même logique pour l'invalidation (on attend que ce soit supprimé)
    let mut invalidated = false;
    for _ in 0..10 {
        if !cache.exists(&cache_key).await? {
            invalidated = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(invalidated, "Cache must be invalidated after save");

    Ok(())
}

#[tokio::test]
async fn test_unique_constraints() -> Result<()> {
    let (repo, pg_ctx, _) = get_test_context().await;
    let identifier = RegistrationIdentifier::try_from_email("unique@test.com")?;

    let acc1 = Account::builder(
        AccountId::generate(RegionCode::default()),
        identifier.clone(),
    )
    .build()?;
    let acc2 = Account::builder(
        AccountId::generate(RegionCode::default()),
        identifier.clone(),
    )
    .build()?;

    let mut tx1 = PostgresTransaction::new(pg_ctx.pool().begin().await.unwrap());
    repo.create(&acc1, &mut tx1).await?;
    tx1.into_inner().commit().await.unwrap();

    // Tentative de doublon
    let mut tx2 = PostgresTransaction::new(pg_ctx.pool().begin().await.unwrap());
    let result = repo.create(&acc2, &mut tx2).await;

    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
async fn test_lookups() -> Result<()> {
    let (repo, pg_ctx, _) = get_test_context().await;
    let email = Email::try_new("lookup@test.com")?;
    let identifier = RegistrationIdentifier::try_from_email(email.to_string())?;
    let ext_id = SubId::from_raw("ext_123");
    let account_id = AccountId::generate(RegionCode::default());

    let account = Account::builder(account_id, identifier)
        .with_sub_id(ext_id.clone())
        .with_email(email.clone())
        .build()?;

    let mut tx = PostgresTransaction::new(pg_ctx.pool().begin().await.unwrap());
    repo.create(&account, &mut tx).await?;
    tx.into_inner().commit().await.unwrap();

    assert!(repo.exists_by_email(&email, None).await?);
    assert!(repo.exists_by_sub_id(&ext_id, None).await?);

    assert_eq!(
        repo.find_id_by_email(&email, None).await?.unwrap(),
        account_id
    );
    assert_eq!(
        repo.find_id_by_sub_id(&ext_id, None).await?.unwrap(),
        account_id
    );

    Ok(())
}

#[tokio::test]
async fn test_rollback_works_properly() -> Result<()> {
    let (repo, pg_ctx, _redis_ctx) = get_test_context().await;
    let account_id = AccountId::generate(RegionCode::default());
    let account = Account::builder(
        account_id,
        RegistrationIdentifier::try_from_email("rollback@test.com")?,
    )
    .build()?;

    let tx_sqlx = pg_ctx.pool().begin().await.unwrap();
    let mut tx = PostgresTransaction::new(tx_sqlx);

    repo.create(&account, &mut tx).await?;
    tx.into_inner().rollback().await.unwrap();

    let found = repo.find_by_id(account_id, None).await?;
    assert!(found.is_none(), "Account should not exist after rollback");

    Ok(())
}

#[tokio::test]
async fn test_cache_hit_proven_by_db_deletion() -> Result<()> {
    let (repo, pg_ctx, _redis_ctx) = get_test_context().await;
    let account_id = AccountId::generate(RegionCode::default());

    let account = Account::builder(
        account_id,
        RegistrationIdentifier::try_from_email("cache-check@test.com")?,
    )
    .build()?;

    // 1. Persistance initiale
    let mut tx = PostgresTransaction::new(pg_ctx.pool().begin().await.unwrap());
    repo.create(&account, &mut tx).await?;
    tx.into_inner().commit().await.unwrap();

    // 2. Premier find_by_id : remplit le cache
    let _ = repo.find_by_id(account_id, None).await?;

    // Attendre le spawn asynchrone du cache
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // 3. SABOTAGE : Suppression SQL directe avec la correction & devant pg_ctx.pool()
    sqlx::query("DELETE FROM account_identity WHERE account_id = $1")
        .bind(account_id.as_uuid())
        .execute(&pg_ctx.pool())
        .await
        .map_err(|e| Error::internal(e.to_string()))?;

    // 4. Tentative de récupération (doit être un Cache Hit)
    let found_from_cache = repo.find_by_id(account_id, None).await?;

    assert!(
        found_from_cache.is_some(),
        "Le cache devrait renvoyer l'objet même si la DB est vide"
    );

    // 5. Verification du bypass en transaction
    let mut tx_check = PostgresTransaction::new(pg_ctx.pool().begin().await.unwrap());
    let found_in_tx = repo.find_by_id(account_id, Some(&mut tx_check)).await?;

    assert!(
        found_in_tx.is_none(),
        "En transaction, on doit ignorer le cache et voir que la DB est vide"
    );

    Ok(())
}

#[tokio::test]
async fn test_cache_performance_benefit() -> Result<()> {
    let (repo, pg_ctx, _redis_ctx) = get_test_context().await;
    let account_id = AccountId::generate(RegionCode::default());

    // On prépare un compte
    let account = Account::builder(
        account_id,
        RegistrationIdentifier::try_from_email("perf@test.com")?,
    )
    .build()?;

    let mut tx = PostgresTransaction::new(pg_ctx.pool().begin().await.unwrap());
    repo.create(&account, &mut tx).await?;
    tx.into_inner().commit().await.unwrap();

    // --- ÉTAPE 1 : Premier appel (Remplit le cache) ---
    let _ = repo.find_by_id(account_id, None).await?;

    // On attend un peu pour être SÛR que le cache est prêt et écrit
    tokio::time::sleep(Duration::from_millis(300)).await;

    // --- ÉTAPE 2 : Mesure de l'appel Cache ---
    let start_cache = std::time::Instant::now();
    let _ = repo.find_by_id(account_id, None).await?;
    let duration_cache = start_cache.elapsed();

    // --- ÉTAPE 3 : Mesure de l'appel DB (en forçant une transaction pour bypass le cache) ---
    let mut tx_force = PostgresTransaction::new(pg_ctx.pool().begin().await.unwrap());
    let start_db = std::time::Instant::now();
    let _ = repo.find_by_id(account_id, Some(&mut tx_force)).await?;
    let duration_db = start_db.elapsed();

    println!("⏱️ Cache: {:?}, ⏱️ DB: {:?}", duration_cache, duration_db);

    // En théorie, Redis est 5x à 10x plus rapide que Postgres avec des JOIN
    assert!(
        duration_cache < duration_db,
        "Le cache doit être plus rapide que la DB"
    );

    Ok(())
}

#[tokio::test]
async fn test_rigorous_partial_fetch_integrity() -> Result<()> {
    let (repo, pg_ctx, _redis_ctx) = get_test_context().await;
    let account_id = AccountId::generate(RegionCode::default());
    let email = Email::try_new("partial@integrity.com")?;

    // --- 1. INSERTION MANUELLE PARTIELLE (SIMULATION BUG/LATENCE) ---
    // On n'insère QUE l'identité, pas les settings ni la gouvernance.
    sqlx::query("INSERT INTO account_identity (account_id, region_code, email, locale, state, version, created_at, updated_at, aggregate_updated_at) 
                 VALUES ($1, $2, $3, 'fr-FR', 'ACTIVE', 0, NOW(), NOW(), NOW())")
        .bind(account_id.as_uuid())
        .bind(account_id.region().to_string())
        .bind(email.to_string())
        .execute(&pg_ctx.pool())
        .await
        .unwrap();

    // --- 2. TENTATIVE DE FETCH DE L'AGRÉGAT COMPLET ---
    let result = repo.find_by_id(account_id, None).await?;
    assert!(
        result.is_some(),
        "Le repo devrait être capable de reconstruire un compte même si les tables satellites sont vides (Audit: Résilience)"
    );

    let account = result.unwrap();
    assert_eq!(
        account.settings().timezone().to_string(),
        "UTC",
        "Devrait avoir une timezone par défaut"
    );

    Ok(())
}
