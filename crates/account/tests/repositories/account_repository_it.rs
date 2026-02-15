// crates/account/tests/repositories/account_repository_it.rs

use account::domain::entities::Account;
use account::domain::repositories::AccountRepository;
use account::infrastructure::postgres::repositories::PostgresAccountRepository;
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
use account::domain::value_objects::{Email, ExternalId, AccountState};
use shared_kernel::domain::events::AggregateRoot;
use uuid::Uuid;
use shared_kernel::infrastructure::postgres::utils::PostgresTestContext;

/// Helper pour instancier le repo et la DB de test
async fn get_test_context() -> (PostgresAccountRepository, PostgresTestContext) {
    let ctx = PostgresTestContext::builder()
        .with_migrations(&["./migrations/postgres"])
        .build()
        .await;

    let repo = PostgresAccountRepository::new(ctx.pool());
    (repo, ctx)
}

#[tokio::test]
async fn test_account_lifecycle_full() {
    let (repo, ctx) = get_test_context().await;
    let pool = ctx.pool();
    let region = RegionCode::try_new("eu").unwrap();
    let account_id = AccountId::new();

    // 1. Création initiale
    let account = Account::builder(
        account_id.clone(),
        region.clone(),
        Username::try_new("sonny_dev").unwrap(),
        Email::try_new("sonny@rust.com").unwrap(),
        ExternalId::try_new(Uuid::now_v7().to_string()).unwrap(),
    ).build();

    let tx_sqlx = pool.begin().await.unwrap();
    let mut wrapped_tx = shared_kernel::infrastructure::postgres::transactions::PostgresTransaction::new(tx_sqlx);

    repo.create_account(&account, &mut wrapped_tx).await.expect("Initial creation failed");
    wrapped_tx.into_inner().commit().await.unwrap();

    // 2. Vérification find_by_id
    let found = repo.find_account_by_id(&account_id, None).await.unwrap().expect("Should find account");
    assert_eq!(found.version(), 1);

    // 3. Update (v1 -> v2)
    let mut to_update = found;
    to_update.deactivate(&region).expect("Deactivation failed");
    repo.save(&to_update, None).await.expect("Save v2 failed");

    let updated = repo.find_account_by_id(&account_id, None).await.unwrap().unwrap();
    assert_eq!(*updated.state(), AccountState::Deactivated);
    assert_eq!(updated.version(), 2);
}

#[tokio::test]
async fn test_transaction_rollback_logic() {
    let (repo, ctx) = get_test_context().await;
    let account_id = AccountId::new();

    let account = Account::builder(
        account_id.clone(),
        RegionCode::try_new("eu").unwrap(),
        Username::try_new("ghost_acc").unwrap(),
        Email::try_new("ghost@void.com").unwrap(),
        ExternalId::try_new(Uuid::now_v7().to_string()).unwrap(),
    ).build();

    let tx_sqlx = ctx.pool().begin().await.unwrap();
    let mut wrapped_tx = shared_kernel::infrastructure::postgres::transactions::PostgresTransaction::new(tx_sqlx);

    repo.create_account(&account, &mut wrapped_tx).await.unwrap();
    wrapped_tx.into_inner().rollback().await.unwrap();

    let found = repo.find_account_by_id(&account_id, None).await.unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn test_unique_constraints_violation() {
    let (repo, _ctx) = get_test_context().await;
    let region = RegionCode::try_new("eu").unwrap();
    let username = "duplicate_user";

    let original = Account::builder(
        AccountId::new(),
        region.clone(),
        Username::try_new(username).unwrap(),
        Email::try_new("first@test.com").unwrap(),
        ExternalId::try_new(Uuid::now_v7().to_string()).unwrap(),
    ).build();

    repo.save(&original, None).await.unwrap();

    let duplicate = Account::builder(
        AccountId::new(),
        region,
        Username::try_new(username).unwrap(),
        Email::try_new("second@test.com").unwrap(),
        ExternalId::try_new(Uuid::now_v7().to_string()).unwrap(),
    ).build();

    let result = repo.save(&duplicate, None).await;
    assert!(result.is_err(), "Duplicate username should trigger unique constraint");
}

#[tokio::test]
async fn test_account_concurrency_conflict_it() {
    let (repo, _ctx) = get_test_context().await;
    let region = RegionCode::try_new("eu").unwrap();
    let account_id = AccountId::new();

    let account = Account::builder(
        account_id.clone(),
        region.clone(),
        Username::try_new("concurrent_user").unwrap(),
        Email::try_new("concurrent@test.com").unwrap(),
        ExternalId::try_new(Uuid::now_v7().to_string()).unwrap(),
    ).build();
    repo.save(&account, None).await.unwrap();

    let mut client_a = repo.find_account_by_id(&account_id, None).await.unwrap().unwrap();
    let mut client_b = repo.find_account_by_id(&account_id, None).await.unwrap().unwrap();

    client_a.deactivate(&region).unwrap();
    repo.save(&client_a, None).await.expect("Client A wins");

    client_b.suspend(&region, "Late update".into()).unwrap();
    let result = repo.save(&client_b, None).await;

    assert!(result.is_err(), "Client B must fail due to OCC mismatch");
}

#[tokio::test]
async fn test_account_security_region_mismatch_it() {
    let (repo, _ctx) = get_test_context().await;

    let region_eu = RegionCode::try_new("eu").unwrap();
    let region_us = RegionCode::try_new("us").unwrap();
    let account_id = AccountId::new();

    let mut account = Account::builder(
        account_id,
        region_eu.clone(),
        Username::try_new("security_test").unwrap(),
        Email::try_new("sec@test.com").unwrap(),
        ExternalId::try_new(Uuid::now_v7().to_string()).unwrap(),
    ).build();

    // 1. Persistance initiale dans la région EU
    repo.save(&account, None).await.unwrap();

    // 2. Tentative de mutation avec la région US
    // Le domaine doit bloquer l'action car l'agrégat appartient à EU (Shard Region Mismatch)
    let result = account.deactivate(&region_us);

    // 3. Assertions
    assert!(result.is_err(), "Le domaine devrait interdire une action sur une région différente");
    assert!(matches!(
        result.unwrap_err(),
        shared_kernel::errors::DomainError::Forbidden { .. }
    ));
}

#[tokio::test]
async fn test_account_lookups() {
    let (repo, _ctx) = get_test_context().await;
    let email = Email::try_new("lookup@test.com").unwrap();
    let username = Username::try_new("lookup_user").unwrap();

    let account = Account::builder(
        AccountId::new(),
        RegionCode::try_new("eu").unwrap(),
        username.clone(),
        email.clone(),
        ExternalId::try_new("ext_999").unwrap(),
    ).build();

    repo.save(&account, None).await.unwrap();

    assert!(repo.exists_account_by_email(&email).await.unwrap());
    assert!(repo.exists_account_by_username(&username).await.unwrap());

    let id = repo.find_account_id_by_email(&email).await.unwrap();
    assert_eq!(id.unwrap(), *account.id());
}