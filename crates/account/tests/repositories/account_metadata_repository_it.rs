// crates/account/tests/repositories/account_metadata_repository_it.rs

use account::domain::entities::AccountMetadata;
use account::domain::repositories::AccountMetadataRepository;
use account::domain::value_objects::AccountRole;
use account::infrastructure::postgres::repositories::PostgresAccountMetadataRepository;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::infrastructure::postgres::transactions::PostgresTransaction;
use uuid::Uuid;

#[tokio::test]
async fn test_metadata_lifecycle_full() {
    let (pool, _c) = crate::common::setup_postgres_test_db().await;
    let repo = PostgresAccountMetadataRepository::new(pool.clone());
    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu".to_string()).unwrap();

    let metadata = AccountMetadata::builder(account_id.clone(), region.clone()).build();
    let mut tx = PostgresTransaction::new(pool.begin().await.unwrap());
    repo.insert(&metadata, &mut tx).await.unwrap();
    tx.into_inner().commit().await.unwrap();

    let mut found = repo.find_by_account_id(&account_id).await.unwrap().unwrap();
    found.upgrade_role(AccountRole::Staff, "Promotion".into()).unwrap();
    found.increase_trust_score(Uuid::now_v7(), 50, "Good behavior".into());

    repo.save(&found, None).await.expect("Save should succeed");

    let final_check = repo.find_by_account_id(&account_id).await.unwrap().unwrap();
    assert_eq!(final_check.role(), AccountRole::Staff);
    assert_eq!(final_check.trust_score(), 50); // 0 + 50 = 50
    assert_eq!(final_check.version(), 3);
}

#[tokio::test]
async fn test_metadata_region_migration_idempotency() {
    let (pool, _c) = crate::common::setup_postgres_test_db().await;
    let repo = PostgresAccountMetadataRepository::new(pool.clone());
    let account_id = AccountId::new();
    let region_eu = RegionCode::try_new("eu").unwrap();
    let region_us = RegionCode::try_new("us").unwrap();

    let metadata = AccountMetadata::builder(account_id.clone(), region_eu).build();
    let mut tx = PostgresTransaction::new(pool.begin().await.unwrap());
    repo.insert(&metadata, &mut tx).await.unwrap();
    tx.into_inner().commit().await.unwrap();

    let mut found = repo.find_by_account_id(&account_id).await.unwrap().unwrap();
    found.change_region(region_us).unwrap();
    repo.save(&found, None).await.unwrap();

    let reloaded = repo.find_by_account_id(&account_id).await.unwrap().unwrap();
    assert_eq!(reloaded.region_code().as_str(), "us");
    assert_eq!(reloaded.version(), 2);
}

#[tokio::test]
async fn test_metadata_concurrency_conflict() {
    let (pool, _c) = crate::common::setup_postgres_test_db().await;
    let repo = PostgresAccountMetadataRepository::new(pool.clone());
    let account_id = AccountId::new();

    let metadata = AccountMetadata::builder(account_id.clone(), RegionCode::try_new("us").unwrap()).build();
    let mut tx = PostgresTransaction::new(pool.begin().await.unwrap());
    repo.insert(&metadata, &mut tx).await.unwrap();
    tx.into_inner().commit().await.unwrap();

    let mut proc_a = repo.find_by_account_id(&account_id).await.unwrap().unwrap();
    let mut proc_b = repo.find_by_account_id(&account_id).await.unwrap().unwrap();

    // Proc A sauve en v2
    proc_a.increase_trust_score(Uuid::now_v7(), 10, "Reward A".into());
    repo.save(&proc_a, None).await.unwrap();

    // Proc B tente de sauver une v2 alors que la DB est déjà en v2
    proc_b.increase_trust_score(Uuid::now_v7(), 10, "Reward B".into());
    let result = repo.save(&proc_b, None).await;

    // Doit échouer car version_db(2) n'est pas < version_rust(2)
    assert!(result.is_err(), "Should fail: version mismatch");
}

#[tokio::test]
async fn test_metadata_auto_shadowban_flow() {
    let (pool, _c) = crate::common::setup_postgres_test_db().await;
    let repo = PostgresAccountMetadataRepository::new(pool.clone());
    let account_id = AccountId::new();

    let metadata = AccountMetadata::builder(account_id.clone(), RegionCode::try_new("eu").unwrap()).build();
    let mut tx = PostgresTransaction::new(pool.begin().await.unwrap());
    repo.insert(&metadata, &mut tx).await.unwrap();
    tx.into_inner().commit().await.unwrap();

    let mut found = repo.find_by_account_id(&account_id).await.unwrap().unwrap();

    // Test règle : score < -20 => shadowban automatique
    found.decrease_trust_score(Uuid::now_v7(), 25, "Botting behavior".into());
    assert!(found.is_shadowbanned());

    repo.save(&found, None).await.unwrap();

    let reloaded = repo.find_by_account_id(&account_id).await.unwrap().unwrap();
    assert!(reloaded.is_shadowbanned());
    assert!(reloaded.moderation_notes().unwrap().contains("Automated system"));
}

#[tokio::test]
async fn test_metadata_transaction_rollback() {
    let (pool, _c) = crate::common::setup_postgres_test_db().await;
    let repo = PostgresAccountMetadataRepository::new(pool.clone());
    let account_id = AccountId::new();

    let tx_sqlx = pool.begin().await.unwrap();
    let mut tx = PostgresTransaction::new(tx_sqlx);

    let metadata = AccountMetadata::builder(account_id.clone(), RegionCode::try_new("eu").unwrap()).build();
    repo.insert(&metadata, &mut tx).await.unwrap();

    // Annulation
    tx.into_inner().rollback().await.unwrap();

    let found = repo.find_by_account_id(&account_id).await.unwrap();
    assert!(found.is_none(), "Metadata should not exist after rollback");
}