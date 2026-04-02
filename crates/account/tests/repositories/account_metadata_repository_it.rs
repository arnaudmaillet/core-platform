// crates/account/tests/repositories/account_metadata_repository_it.rs

use account::domain::entities::account::AccountMetadata;
use account::domain::repositories::AccountMetadataRepository;
use account::domain::value_objects::AccountRole;
use account::infrastructure::postgres::repositories::PostgresAccountMetadataRepository;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::infrastructure::postgres::transactions::PostgresTransaction;
use shared_kernel::infrastructure::postgres::utils::PostgresTestContext;
use uuid::Uuid;

async fn get_pg_context() -> PostgresTestContext {
    PostgresTestContext::builder()
        .with_migrations(&["./migrations/postgres"])
        .build()
        .await
}

#[tokio::test]
async fn test_metadata_lifecycle_full() {
    let ctx = get_pg_context().await;
    let pool = ctx.pool();
    let repo = PostgresAccountMetadataRepository::new(pool.clone());

    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();

    let metadata = AccountMetadata::builder(account_id.clone(), region.clone())
        .with_trust_score(100)
        .build();

    repo.save(&metadata, None, None)
        .await
        .expect("Initial save failed");

    let mut found = repo
        .fetch_by_account_id(&account_id)
        .await
        .unwrap()
        .expect("Should find metadata");
    let original = found.clone();

    found
        .upgrade_role(&region, AccountRole::Moderator, "Promotion".into())
        .unwrap();
    found
        .decrease_trust_score(&region, Uuid::now_v7(), 50, "Warning".into())
        .unwrap();

    repo.save(&found, Some(&original), None)
        .await
        .expect("Save should succeed");

    let final_check = repo
        .fetch_by_account_id(&account_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(final_check.role(), AccountRole::Moderator);
    assert_eq!(final_check.trust_score(), 50);
    assert_eq!(final_check.version(), 3); // v1 + upgrade + decrease = v3
}

#[tokio::test]
async fn test_metadata_region_migration_idempotency() {
    let ctx = get_pg_context().await;
    let repo = PostgresAccountMetadataRepository::new(ctx.pool().clone());

    let account_id = AccountId::new();
    let region_eu = RegionCode::try_new("eu").unwrap();
    let region_us = RegionCode::try_new("us").unwrap();

    // 1. Création initiale en EU
    let metadata = AccountMetadata::builder(account_id.clone(), region_eu.clone()).build();
    repo.save(&metadata, None, None).await.unwrap();

    // 2. Récupération
    let found = repo
        .fetch_by_account_id(&account_id)
        .await
        .unwrap()
        .unwrap();

    // 3. Migration vers US
    let original = found.clone(); // ON GARDE UNE COPIE DE L'ÉTAT "EU"
    let mut updated = found;
    updated.change_region(region_us.clone()).unwrap();

    // On passe l'original pour que le DELETE de l'étape 1 du repo se déclenche
    repo.save(&updated, Some(&original), None).await.unwrap();

    // 4. Vérification
    let reloaded = repo
        .fetch_by_account_id(&account_id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(reloaded.region_code().as_str(), "us");
    assert_eq!(reloaded.version(), 2); // La version a dû incrémenter
}

#[tokio::test]
async fn test_metadata_concurrency_conflict() {
    let ctx = get_pg_context().await;
    let repo = PostgresAccountMetadataRepository::new(ctx.pool().clone());

    let account_id = AccountId::new();
    let region = RegionCode::try_new("us").unwrap();

    let metadata = AccountMetadata::builder(account_id.clone(), region.clone())
        .with_trust_score(50)
        .build();
    repo.save(&metadata, None, None).await.unwrap();

    // Deux clients chargent la version 1
    let client_a_found = repo
        .fetch_by_account_id(&account_id)
        .await
        .unwrap()
        .unwrap();
    let client_b_found = repo
        .fetch_by_account_id(&account_id)
        .await
        .unwrap()
        .unwrap();

    // Client A gagne (v1 -> v2)
    let mut proc_a = client_a_found.clone();
    proc_a
        .increase_trust_score(&region, Uuid::now_v7(), 10, "Reward A".into())
        .unwrap();
    repo.save(&proc_a, Some(&client_a_found), None)
        .await
        .unwrap();

    // Client B échoue (tente v1 -> v2 mais la DB est en v2)
    let mut proc_b = client_b_found.clone();
    proc_b
        .increase_trust_score(&region, Uuid::now_v7(), 10, "Reward B".into())
        .unwrap();
    let result = repo.save(&proc_b, Some(&client_b_found), None).await;

    assert!(
        result.is_err(),
        "Should fail due to Optimistic Concurrency Control (OCC)"
    );
    assert!(matches!(
        result.unwrap_err(),
        shared_kernel::errors::DomainError::ConcurrencyConflict { .. }
    ));
}

#[tokio::test]
async fn test_metadata_auto_shadowban_flow() {
    let ctx = get_pg_context().await;
    let repo = PostgresAccountMetadataRepository::new(ctx.pool().clone());
    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();
    let metadata = AccountMetadata::builder(account_id.clone(), region.clone())
        .with_trust_score(100)
        .build();
    repo.save(&metadata, None, None).await.unwrap();

    let mut found = repo
        .fetch_by_account_id(&account_id)
        .await
        .unwrap()
        .unwrap();
    let original = found.clone();

    found
        .decrease_trust_score(&region, Uuid::now_v7(), 100, "Botting".into())
        .unwrap();
    assert!(found.is_shadowbanned());

    repo.save(&found, Some(&original), None).await.unwrap();

    let reloaded = repo
        .fetch_by_account_id(&account_id)
        .await
        .unwrap()
        .unwrap();
    assert!(reloaded.is_shadowbanned());
}

#[tokio::test]
async fn test_metadata_transaction_rollback() {
    let ctx = get_pg_context().await;
    let pool = ctx.pool();
    let repo = PostgresAccountMetadataRepository::new(pool.clone());

    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();

    let tx_sqlx = pool.begin().await.unwrap();
    let mut tx = PostgresTransaction::new(tx_sqlx);

    let metadata = AccountMetadata::builder(account_id.clone(), region).build();
    // Utilisation de la transaction pour le save
    repo.save(&metadata, None, Some(&mut tx)).await.unwrap();

    tx.into_inner().rollback().await.unwrap();

    let found = repo.fetch_by_account_id(&account_id).await.unwrap();
    assert!(found.is_none(), "Metadata should not exist after rollback");
}
