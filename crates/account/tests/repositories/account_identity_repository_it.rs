// crates/account/tests/repositories/account_repository_it.rs

use account::domain::account::entities::AccountIdentity;
use account::domain::repositories::AccountIdentityRepository;
use account::domain::value_objects::{AccountState, Email, ExternalId};
use account::infrastructure::postgres::repositories::PostgresAccountIdentityRepository;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::infrastructure::postgres::transactions::PostgresTransaction;
use shared_kernel::infrastructure::postgres::utils::PostgresTestContext;
use shared_kernel::infrastructure::redis::utils::RedisTestContext;
use shared_kernel::domain::repositories::CacheRepository;

/// Helper pour instancier le repo et la DB de test
async fn get_test_context() -> (PostgresAccountIdentityRepository, PostgresTestContext, RedisTestContext) {
    // 1. Démarrage de Postgres (Docker)
    let pg_ctx = PostgresTestContext::builder()
        .with_migrations(&["./migrations/postgres"])
        .build()
        .await;

    // 2. Démarrage de Redis (Docker via ton nouveau util)
    let redis_ctx = RedisTestContext::builder()
        .build()
        .await;

    // 3. Instanciation du Repo avec les deux containers
    let repo = PostgresAccountIdentityRepository::new(
        pg_ctx.pool().clone(), 
        redis_ctx.repository()
    );
    
    (repo, pg_ctx, redis_ctx)
}

#[tokio::test]
async fn test_cache_invalidation_lifecycle() {
    let (repo, _ctx, redis_ctx) = get_test_context().await;
    let account_id = AccountId::new();
    let email = Email::try_new("cache@test.com").unwrap();
    let cache = redis_ctx.repository();

    let account = AccountIdentity::builder(
        account_id.clone(),
        RegionCode::try_new("eu").unwrap(),
        email.clone(),
        ExternalId::from_raw("ext_cache"),
    ).build();

    // 1. Sauvegarde initiale
    repo.save(&account, None, None).await.unwrap();

    // 2. Premier Fetch -> Doit remplir le cache
    let found_1 = repo.fetch_by_account_id(&account_id, None).await.unwrap().unwrap();
    
    // Vérification manuelle que c'est dans Redis
    let cache_key = format!("account:identity:{}", account_id.clone());
    let in_cache: bool = cache.exists(&cache_key).await.unwrap();
    assert!(in_cache, "Data should be in cache after first fetch");

    // 3. Modification (v1 -> v2) -> Doit INVALIDER le cache
    let mut to_update = found_1.clone();
    to_update.deactivate().unwrap();
    repo.save(&to_update, Some(&found_1), None).await.unwrap();

    // Vérification que la clé a disparu de Redis
    let in_cache_after_save: bool = cache.exists(&cache_key).await.unwrap();
    assert!(!in_cache_after_save, "Cache should be invalidated after save");

    // 4. Second Fetch -> Doit renvoyer la v2 et re-remplir le cache
    let found_2 = repo.fetch_by_account_id(&account_id, None).await.unwrap().unwrap();
    assert_eq!(found_2.version(), 2);
    assert!(cache.exists(&cache_key).await.unwrap(), "Cache should be refilled");
}

#[tokio::test]
async fn test_transaction_skips_cache() {
    let (repo, ctx, redis_ctx) = get_test_context().await;
    let cache = redis_ctx.repository();
    let account_id = AccountId::new();

    let account = AccountIdentity::builder(
        account_id.clone(),
        RegionCode::try_new("eu").unwrap(),
        Email::try_new("tx_cache@test.com").unwrap(),
        ExternalId::from_raw("ext_tx_cache"),
    ).build();

    // On sauve en DB
    repo.save(&account, None, None).await.unwrap();

    // On ouvre une transaction
    let tx_sqlx = ctx.pool().begin().await.unwrap();
    let mut wrapped_tx = PostgresTransaction::new(tx_sqlx);

    // On fetch DANS une transaction
    // Selon ton code : tx.is_some() donc ça ne devrait pas lire le cache 
    // et surtout ça ne devrait pas ÉCRIRE dans le cache (Dirty Read protection)
    let _ = repo.fetch_by_account_id(&account_id, Some(&mut wrapped_tx)).await.unwrap();

    let cache_key = format!("account:identity:{}", account_id.clone());
    assert!(!cache.exists(&cache_key).await.unwrap(), "Cache should not be filled during a transaction");
}

#[tokio::test]
async fn test_account_lifecycle_full() {
    let (repo, ctx, _redis_ctx) = get_test_context().await;
    let pool = ctx.pool();
    let region = RegionCode::try_new("eu").unwrap();
    let account_id = AccountId::new();

    // 1. Création initiale (Version 1)
    let account = AccountIdentity::builder(
        account_id.clone(),
        region.clone(),
        Email::try_new("sonny@rust.com").unwrap(),
        ExternalId::from_raw("ext_lifecycle"),
    )
    .build();

    // Utilisation du nouveau système de transaction
    let tx_sqlx = pool.begin().await.unwrap();
    let mut wrapped_tx = PostgresTransaction::new(tx_sqlx);

    // Note: save() gère l'insert si original est None
    repo.save(&account, None, Some(&mut wrapped_tx))
        .await
        .expect("Initial creation failed");
    wrapped_tx.into_inner().commit().await.unwrap();

    // 2. Vérification fetch_by_id
    let found = repo
        .fetch_by_account_id(&account_id, None)
        .await
        .unwrap()
        .expect("Should find account");
    assert_eq!(found.version(), 1);

    // 3. Update (v1 -> v2)
    let mut to_update = found.clone();
    to_update.deactivate().expect("Deactivation failed");

    // On passe 'found' comme original pour activer le verrouillage optimiste
    repo.save(&to_update, Some(&found), None)
        .await
        .expect("Save v2 failed");

    let updated = repo.fetch_by_account_id(&account_id, None).await.unwrap().unwrap();
    assert_eq!(*updated.state(), AccountState::Deactivated);
    assert_eq!(updated.version(), 2);
}

#[tokio::test]
async fn test_transaction_rollback_logic() {
    let (repo, ctx, _redis_ctx) = get_test_context().await;
    let account_id = AccountId::new();

    let account = AccountIdentity::builder(
        account_id.clone(),
        RegionCode::try_new("eu").unwrap(),
        Email::try_new("ghost@void.com").unwrap(),
        ExternalId::from_raw("ext_ghost"),
    )
    .build();

    let tx_sqlx = ctx.pool().begin().await.unwrap();
    let mut wrapped_tx = PostgresTransaction::new(tx_sqlx);

    repo.save(&account, None, Some(&mut wrapped_tx))
        .await
        .unwrap();
    wrapped_tx.into_inner().rollback().await.unwrap();

    let found = repo.fetch_by_account_id(&account_id, None).await.unwrap();
    assert!(found.is_none(), "Account should not exist after rollback");
}

#[tokio::test]
async fn test_unique_constraints_violation() {
    let (repo, _ctx, _redis_ctx) = get_test_context().await;
    let region = RegionCode::try_new("eu").unwrap();
    let email_str = "duplicate@test.com";

    let original = AccountIdentity::builder(
        AccountId::new(),
        region.clone(),
        Email::try_new(email_str).unwrap(),
        ExternalId::from_raw("ext_1"),
    )
    .build();

    repo.save(&original, None, None).await.unwrap();

    let duplicate = AccountIdentity::builder(
        AccountId::new(),
        region,
        Email::try_new(email_str).unwrap(), // Même email
        ExternalId::from_raw("ext_2"),
    )
    .build();

    let result = repo.save(&duplicate, None, None).await;
    assert!(
        result.is_err(),
        "Duplicate email should trigger unique constraint"
    );
}

#[tokio::test]
async fn test_account_concurrency_conflict_it() {
    let (repo, _ctx, _redis_ctx) = get_test_context().await;
    let region = RegionCode::try_new("eu").unwrap();
    let account_id = AccountId::new();

    let account = AccountIdentity::builder(
        account_id.clone(),
        region.clone(),
        Email::try_new("concurrent@test.com").unwrap(),
        ExternalId::from_raw("ext_concurrent"),
    )
    .build();
    repo.save(&account, None, None).await.unwrap();

    // On simule deux clients qui chargent la même version (v1)
    let client_a_found = repo.fetch_by_account_id(&account_id, None).await.unwrap().unwrap();
    let client_b_found = repo.fetch_by_account_id(&account_id, None).await.unwrap().unwrap();

    // Client A sauvegarde en premier (v1 -> v2)
    let mut client_a_modified = client_a_found.clone();
    client_a_modified.deactivate().unwrap();
    repo.save(&client_a_modified, Some(&client_a_found), None)
        .await
        .expect("Client A wins");

    // Client B essaie de sauvegarder sa version v1 (v1 -> v2) MAIS la DB est déjà en v2
    let mut client_b_modified = client_b_found.clone();
    client_b_modified
        .suspend("Late update".into())
        .unwrap();

    let result = repo
        .save(&client_b_modified, Some(&client_b_found), None)
        .await;

    // L'erreur doit être un ConcurrencyConflict
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        shared_kernel::errors::DomainError::ConcurrencyConflict { .. }
    ));
}

#[tokio::test]
async fn test_account_lookups_and_resolutions() {
    let (repo, _ctx, _redis_ctx) = get_test_context().await;
    let email = Email::try_new("lookup@test.com").unwrap();
    let ext_id = ExternalId::from_raw("ext_lookup_123");

    let account = AccountIdentity::builder(
        AccountId::new(),
        RegionCode::try_new("eu").unwrap(),
        email.clone(),
        ext_id.clone(),
    )
    .build();

    repo.save(&account, None, None).await.unwrap();

    // Test des méthodes de vérification
    assert!(repo.exists_by_email(&email).await.unwrap());
    assert!(repo.exists_by_external_id(&ext_id).await.unwrap());

    // Test des résolutions d'ID
    let id_from_email = repo.resolve_id_from_email(&email).await.unwrap();
    assert_eq!(id_from_email.unwrap(), *account.account_id());

    let id_from_ext = repo.resolve_id_from_external_id(&ext_id).await.unwrap();
    assert_eq!(id_from_ext.unwrap(), *account.account_id());
}