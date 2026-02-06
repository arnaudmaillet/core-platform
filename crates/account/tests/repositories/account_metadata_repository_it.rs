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

    // Mise à jour du contrat : on passe la région
    found.upgrade_role(&region, AccountRole::Staff, "Promotion".into()).unwrap();
    found.increase_trust_score(&region, Uuid::now_v7(), 50, "Good behavior".into()).unwrap();

    repo.save(&found, None).await.expect("Save should succeed");

    let final_check = repo.find_by_account_id(&account_id).await.unwrap().unwrap();
    assert_eq!(final_check.role(), AccountRole::Staff);
    // Score par défaut (50) + 50 = 100 (cap) ou selon ta logique initiale
    assert_eq!(final_check.trust_score(), 100);
    assert_eq!(final_check.version(), 3);
}

#[tokio::test]
async fn test_metadata_region_migration_idempotency() {
    let (pool, _c) = crate::common::setup_postgres_test_db().await;
    let repo = PostgresAccountMetadataRepository::new(pool.clone());
    let account_id = AccountId::new();
    let region_eu = RegionCode::try_new("eu").unwrap();
    let region_us = RegionCode::try_new("us").unwrap();

    let metadata = AccountMetadata::builder(account_id.clone(), region_eu.clone()).build();
    let mut tx = PostgresTransaction::new(pool.begin().await.unwrap());
    repo.insert(&metadata, &mut tx).await.unwrap();
    tx.into_inner().commit().await.unwrap();

    let mut found = repo.find_by_account_id(&account_id).await.unwrap().unwrap();

    // Changement de région (opération admin/système)
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
    let region = RegionCode::try_new("us").unwrap();

    let metadata = AccountMetadata::builder(account_id.clone(), region.clone()).build();
    let mut tx = PostgresTransaction::new(pool.begin().await.unwrap());
    repo.insert(&metadata, &mut tx).await.unwrap();
    tx.into_inner().commit().await.unwrap();

    let mut proc_a = repo.find_by_account_id(&account_id).await.unwrap().unwrap();
    let mut proc_b = repo.find_by_account_id(&account_id).await.unwrap().unwrap();

    // Proc A sauve en v2
    proc_a.increase_trust_score(&region, Uuid::now_v7(), 10, "Reward A".into()).unwrap();
    repo.save(&proc_a, None).await.unwrap();

    // Proc B tente de sauver une v2 alors que la DB est déjà en v2 (version_db = 2)
    proc_b.increase_trust_score(&region, Uuid::now_v7(), 10, "Reward B".into()).unwrap();
    let result = repo.save(&proc_b, None).await;

    // L'OCC (Optimistic Concurrency Control) doit rejeter le save de B
    assert!(result.is_err(), "Should fail: OCC version mismatch");
}

#[tokio::test]
async fn test_metadata_auto_shadowban_flow() {
    let (pool, _c) = crate::common::setup_postgres_test_db().await;
    let repo = PostgresAccountMetadataRepository::new(pool.clone());
    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();

    let metadata = AccountMetadata::builder(account_id.clone(), region.clone()).build();
    let mut tx = PostgresTransaction::new(pool.begin().await.unwrap());
    repo.insert(&metadata, &mut tx).await.unwrap();
    tx.into_inner().commit().await.unwrap();

    let mut found = repo.find_by_account_id(&account_id).await.unwrap().unwrap();

    // Baisse drastique du score pour trigger le shadowban
    found.decrease_trust_score(&region, Uuid::now_v7(), 80, "Botting behavior".into()).unwrap();
    assert!(found.is_shadowbanned());

    repo.save(&found, None).await.unwrap();

    let reloaded = repo.find_by_account_id(&account_id).await.unwrap().unwrap();
    assert!(reloaded.is_shadowbanned());
}

#[tokio::test]
async fn test_metadata_transaction_rollback() {
    let (pool, _c) = crate::common::setup_postgres_test_db().await;
    let repo = PostgresAccountMetadataRepository::new(pool.clone());
    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();

    let tx_sqlx = pool.begin().await.unwrap();
    let mut tx = PostgresTransaction::new(tx_sqlx);

    let metadata = AccountMetadata::builder(account_id.clone(), region).build();
    repo.insert(&metadata, &mut tx).await.unwrap();

    // Annulation de la transaction SQL
    tx.into_inner().rollback().await.unwrap();

    let found = repo.find_by_account_id(&account_id).await.unwrap();
    assert!(found.is_none(), "Metadata should not exist after rollback");
}