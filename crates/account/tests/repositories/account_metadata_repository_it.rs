// crates/account/tests/repositories/account_metadata_repository_it.rs

use account::domain::account::entities::AccountMetadata;
use account::domain::repositories::AccountMetadataRepository;
use account::domain::value_objects::AccountRole;
use account::infrastructure::postgres::repositories::PostgresAccountMetadataRepository;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::domain::repositories::CacheRepository;
use shared_kernel::infrastructure::postgres::transactions::PostgresTransaction;
use shared_kernel::infrastructure::postgres::utils::PostgresTestContext;
use shared_kernel::infrastructure::redis::utils::RedisTestContext;
use uuid::Uuid;

async fn get_test_context() -> (PostgresAccountMetadataRepository, PostgresTestContext, RedisTestContext) {
    // 1. Postgres
    let pg_ctx = PostgresTestContext::builder()
        .with_migrations(&["./migrations/postgres"])
        .build()
        .await;

    // 2. Redis
    let redis_ctx = RedisTestContext::builder()
        .build()
        .await;

    // 3. Repo avec injection du cache
    let repo = PostgresAccountMetadataRepository::new(
        pg_ctx.pool().clone(), 
        redis_ctx.repository()
    );
    
    (repo, pg_ctx, redis_ctx)
}

#[tokio::test]
async fn test_metadata_cache_invalidation_flow() {
    let (repo, _pg_ctx, redis_ctx) = get_test_context().await;
    let cache = redis_ctx.repository();
    let account_id = AccountId::new();
    let cache_key = format!("account:metadata:{}", account_id.clone());

    let metadata = AccountMetadata::builder(account_id.clone())
        .with_trust_score(100)
        .build();

    // 1. Sauvegarde initiale
    repo.save(&metadata, None, None).await.unwrap();

    // 2. Premier Fetch -> Doit remplir le cache
    let found = repo.fetch_by_account_id(&account_id).await.unwrap().unwrap();
    assert!(cache.exists(&cache_key).await.unwrap(), "Metadata should be cached");

    // 3. Modification (v1 -> v2) -> Doit supprimer la clé Redis
    let mut to_update = found.clone();
    to_update.upgrade_role(AccountRole::Moderator, "Test".into()).unwrap();
    
    repo.save(&to_update, Some(&found), None).await.unwrap();

    // VÉRIFICATION CRITIQUE : La clé doit avoir disparu
    let in_cache = cache.exists(&cache_key).await.unwrap();
    assert!(!in_cache, "Cache MUST be invalidated after metadata update");

    // 4. Second Fetch -> Doit re-remplir le cache avec la v2
    let final_check = repo.fetch_by_account_id(&account_id).await.unwrap().unwrap();
    assert_eq!(final_check.role(), AccountRole::Moderator);
    assert!(cache.exists(&cache_key).await.unwrap(), "Cache should be refilled after fetch");
}

#[tokio::test]
async fn test_metadata_lifecycle_full() {
    let (repo, _pg_ctx, _redis_ctx) = get_test_context().await;
    let account_id = AccountId::new();

    let metadata = AccountMetadata::builder(account_id.clone())
        .with_trust_score(100)
        .build();

    repo.save(&metadata, None, None).await.expect("Initial save failed");

    let mut found = repo.fetch_by_account_id(&account_id).await.unwrap().expect("Should find metadata");
    let original = found.clone();

    found.upgrade_role(AccountRole::Moderator, "Promotion".into()).unwrap();
    found.decrease_trust_score(Uuid::now_v7(), 50, "Warning".into()).unwrap();

    repo.save(&found, Some(&original), None).await.expect("Save should succeed");

    let final_check = repo.fetch_by_account_id(&account_id).await.unwrap().unwrap();
    assert_eq!(final_check.role(), AccountRole::Moderator);
    assert_eq!(final_check.trust_score(), 50);
    assert_eq!(final_check.version(), 3);
}

#[tokio::test]
async fn test_metadata_concurrency_conflict() {
    let (repo, _pg_ctx, _redis_ctx) = get_test_context().await;
    let account_id = AccountId::new();

    let metadata = AccountMetadata::builder(account_id.clone()).with_trust_score(50).build();
    repo.save(&metadata, None, None).await.unwrap();

    let client_a_found = repo.fetch_by_account_id(&account_id).await.unwrap().unwrap();
    let client_b_found = repo.fetch_by_account_id(&account_id).await.unwrap().unwrap();

    // Client A gagne
    let mut proc_a = client_a_found.clone();
    proc_a.increase_trust_score(Uuid::now_v7(), 10, "Reward A".into()).unwrap();
    repo.save(&proc_a, Some(&client_a_found), None).await.unwrap();

    // Client B échoue
    let mut proc_b = client_b_found.clone();
    proc_b.increase_trust_score(Uuid::now_v7(), 10, "Reward B".into()).unwrap();
    let result = repo.save(&proc_b, Some(&client_b_found), None).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), shared_kernel::errors::DomainError::ConcurrencyConflict { .. }));
}

#[tokio::test]
async fn test_metadata_auto_shadowban_flow() {
    // 1. Initialisation avec Postgres + Redis (Docker)
    let (repo, _pg_ctx, redis_ctx) = get_test_context().await;
    let cache = redis_ctx.repository();
    let account_id = AccountId::new();
    let cache_key = format!("account:metadata:{}", account_id.clone());

    let metadata = AccountMetadata::builder(account_id.clone())
        .with_trust_score(100)
        .build();
    repo.save(&metadata, None, None).await.unwrap();

    // 2. Premier chargement (remplit le cache en version "saine")
    let found = repo
        .fetch_by_account_id(&account_id)
        .await
        .unwrap()
        .unwrap();
    assert!(!found.is_shadowbanned());
    assert!(cache.exists(&cache_key).await.unwrap());

    // 3. Application de la sanction (Logique du Domaine)
    let mut modified = found.clone();
    modified
        .decrease_trust_score(Uuid::now_v7(), 100, "Botting".into())
        .unwrap();
    
    // On vérifie que l'objet en mémoire est bien shadowbanned
    assert!(modified.is_shadowbanned());

    // 4. Sauvegarde -> Doit persister en DB ET invalider le cache "sain"
    repo.save(&modified, Some(&found), None).await.unwrap();
    
    // Vérification que le cache est vide (pour ne pas servir l'ancien état "non-banni")
    assert!(!cache.exists(&cache_key).await.unwrap(), "Cache must be cleared to apply shadowban immediately");

    // 5. Rechargement -> Doit venir de la DB et confirmer le shadowban
    let reloaded = repo
        .fetch_by_account_id(&account_id)
        .await
        .unwrap()
        .unwrap();
        
    assert!(reloaded.is_shadowbanned());
    assert_eq!(reloaded.trust_score(), 0);
}

#[tokio::test]
async fn test_metadata_transaction_rollback() {
    let (repo, pg_ctx, redis_ctx) = get_test_context().await;
    let pool = pg_ctx.pool();
    let cache = redis_ctx.repository();
    let account_id = AccountId::new();

    let tx_sqlx = pool.begin().await.unwrap();
    let mut tx = PostgresTransaction::new(tx_sqlx);

    let metadata = AccountMetadata::builder(account_id.clone()).build();
    repo.save(&metadata, None, Some(&mut tx)).await.unwrap();

    tx.into_inner().rollback().await.unwrap();

    // On vérifie que rien n'est en base
    let found_db = repo.fetch_by_account_id(&account_id).await.unwrap();
    assert!(found_db.is_none());

    // On vérifie que rien n'a été mis en cache (Dirty Write protection)
    let cache_key = format!("account:metadata:{}", account_id.clone());
    assert!(!cache.exists(&cache_key).await.unwrap());
}