// crates/account/tests/repositories/account_metadata_repository_it.rs

use account::domain::entities::AccountMetadata;
use account::domain::repositories::AccountMetadataRepository;
use account::domain::value_objects::AccountRole;
use account::infrastructure::postgres::repositories::PostgresAccountMetadataRepository;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::infrastructure::postgres::transactions::PostgresTransaction;
use uuid::Uuid;
use shared_kernel::infrastructure::postgres::utils::PostgresTestContext;

async fn get_pg_context() -> PostgresTestContext {
    PostgresTestContext::builder()
        .with_migrations(&["./migrations/postgres"]) // Chemins relatifs au module account
        .build()
        .await
}

async fn test_metadata_lifecycle_full() {
    let ctx = get_pg_context().await;
    let pool = ctx.pool();
    let repo = PostgresAccountMetadataRepository::new(pool.clone());

    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu".to_string()).unwrap();

    let metadata = AccountMetadata::builder(account_id.clone(), region.clone()).build();
    let mut tx = PostgresTransaction::new(pool.begin().await.unwrap());
    repo.insert(&metadata, &mut tx).await.unwrap();
    tx.into_inner().commit().await.unwrap();

    let mut found = repo.find_by_account_id(&account_id).await.unwrap().unwrap();

    found.upgrade_role(&region, AccountRole::Staff, "Promotion".into()).unwrap();
    found.increase_trust_score(&region, Uuid::now_v7(), 50, "Good behavior".into()).unwrap();

    repo.save(&found, None).await.expect("Save should succeed");

    let final_check = repo.find_by_account_id(&account_id).await.unwrap().unwrap();
    assert_eq!(final_check.role(), AccountRole::Staff);
    assert_eq!(final_check.trust_score(), 100);
    assert_eq!(final_check.version(), 3);
}

#[tokio::test]
async fn test_metadata_region_migration_idempotency() {
    let ctx = get_pg_context().await;
    let pool = ctx.pool();
    let repo = PostgresAccountMetadataRepository::new(pool.clone());

    let account_id = AccountId::new();
    let region_eu = RegionCode::try_new("eu").unwrap();
    let region_us = RegionCode::try_new("us").unwrap();

    let metadata = AccountMetadata::builder(account_id.clone(), region_eu.clone()).build();
    let mut tx = PostgresTransaction::new(pool.begin().await.unwrap());
    repo.insert(&metadata, &mut tx).await.unwrap();
    tx.into_inner().commit().await.unwrap();

    let mut found = repo.find_by_account_id(&account_id).await.unwrap().unwrap();

    found.change_region(region_us).unwrap();
    repo.save(&found, None).await.unwrap();

    let reloaded = repo.find_by_account_id(&account_id).await.unwrap().unwrap();
    assert_eq!(reloaded.region_code().as_str(), "us");
}

#[tokio::test]
async fn test_metadata_concurrency_conflict() {
    let ctx = get_pg_context().await;
    let pool = ctx.pool();
    let repo = PostgresAccountMetadataRepository::new(pool.clone());

    let account_id = AccountId::new();
    let region = RegionCode::try_new("us").unwrap();

    let metadata = AccountMetadata::builder(account_id.clone(), region.clone()).build();
    let mut tx = PostgresTransaction::new(pool.begin().await.unwrap());
    repo.insert(&metadata, &mut tx).await.unwrap();
    tx.into_inner().commit().await.unwrap();

    let mut proc_a = repo.find_by_account_id(&account_id).await.unwrap().unwrap();
    let mut proc_b = repo.find_by_account_id(&account_id).await.unwrap().unwrap();

    proc_a.increase_trust_score(&region, Uuid::now_v7(), 10, "Reward A".into()).unwrap();
    repo.save(&proc_a, None).await.unwrap();

    proc_b.increase_trust_score(&region, Uuid::now_v7(), 10, "Reward B".into()).unwrap();
    let result = repo.save(&proc_b, None).await;

    assert!(result.is_err(), "Should fail due to Optimistic Concurrency Control");
}

#[tokio::test]
async fn test_metadata_auto_shadowban_flow() {
    let ctx = get_pg_context().await;
    let pool = ctx.pool();
    let repo = PostgresAccountMetadataRepository::new(pool.clone());

    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();

    // 1. Insertion initiale
    let metadata = AccountMetadata::builder(account_id.clone(), region.clone()).build();
    let mut tx = PostgresTransaction::new(pool.begin().await.unwrap());
    repo.insert(&metadata, &mut tx).await.unwrap();
    tx.into_inner().commit().await.unwrap();

    // 2. Récupération et modification du score de confiance
    let mut found = repo.find_by_account_id(&account_id).await.unwrap().unwrap();

    // Baisse drastique du score pour trigger la logique métier de shadowban
    found.decrease_trust_score(&region, Uuid::now_v7(), 80, "Botting behavior".into()).unwrap();
    assert!(found.is_shadowbanned(), "L'entité devrait être shadowbanned en mémoire");

    // 3. Persistance du nouvel état
    repo.save(&found, None).await.unwrap();

    // 4. Vérification après rechargement depuis la DB
    let reloaded = repo.find_by_account_id(&account_id).await.unwrap().unwrap();
    assert!(reloaded.is_shadowbanned(), "L'état shadowbanned devrait être persisté en base de données");
}

#[tokio::test]
async fn test_metadata_transaction_rollback() {
    let ctx = get_pg_context().await;
    let pool = ctx.pool();
    let repo = PostgresAccountMetadataRepository::new(pool.clone());

    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu".to_string()).unwrap();

    let tx_sqlx = pool.begin().await.unwrap();
    let mut tx = PostgresTransaction::new(tx_sqlx);

    let metadata = AccountMetadata::builder(account_id.clone(), region).build();
    repo.insert(&metadata, &mut tx).await.unwrap();

    tx.into_inner().rollback().await.unwrap();

    let found = repo.find_by_account_id(&account_id).await.unwrap();
    assert!(found.is_none());
}