// crates/account/tests/repositories/account_repository_it.rs

use account::domain::entities::Account;
use account::domain::repositories::AccountRepository;
use account::infrastructure::postgres::repositories::PostgresAccountRepository;
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
use account::domain::value_objects::{Email, ExternalId, AccountState};
use uuid::Uuid;

async fn get_repo() -> (
    PostgresAccountRepository,
    testcontainers::ContainerAsync<testcontainers_modules::postgres::Postgres>,
) {
    let (pool, container) = crate::common::setup_postgres_test_db().await;
    (PostgresAccountRepository::new(pool), container)
}

#[tokio::test]
async fn test_account_lifecycle() {
    // 1. On récupère la pool et le container via ton setup commun
    let (pool, _c) = crate::common::setup_postgres_test_db().await;

    // 2. On crée le repo manuellement avec cette pool
    let repo = PostgresAccountRepository::new(pool.clone());

    let account = Account::builder(
        AccountId::new(),
        RegionCode::try_new("eu".to_string()).unwrap(),
        Username::try_new("sonny_dev".to_string()).unwrap(),
        Email::try_new("sonny@rust.com".to_string()).unwrap(),
        ExternalId::try_new(Uuid::now_v7().to_string()).unwrap(),
    ).build();

    // 3. MAINTENANT tu peux utiliser 'pool' pour démarrer la transaction
    let tx_sqlx = pool.begin().await.unwrap();
    let mut wrapped_tx = shared_kernel::infrastructure::postgres::transactions::PostgresTransaction::new(tx_sqlx);

    repo.create_account(&account, &mut wrapped_tx).await.unwrap();
    wrapped_tx.into_inner().commit().await.unwrap();

    // 4. Vérification
    let found = repo.find_account_by_id(account.id(), None).await.unwrap().expect("Should find account");
    assert_eq!(found.username().as_str(), "sonny_dev");
}

#[tokio::test]
async fn test_transaction_rollback_logic() {
    let (pool, _c) = crate::common::setup_postgres_test_db().await;
    let repo = PostgresAccountRepository::new(pool.clone());

    let account = Account::builder(
        AccountId::new(),
        RegionCode::try_new("eu".to_string()).unwrap(),
        Username::try_new("ghost_acc".to_string()).unwrap(),
        Email::try_new("ghost@void.com".to_string()).unwrap(),
        ExternalId::try_new(Uuid::now_v7().to_string()).unwrap(),
    ).build();

    // 1. Début transaction brute
    let tx_sqlx = pool.begin().await.unwrap();
    // 2. Wrapper
    let mut wrapped_tx = shared_kernel::infrastructure::postgres::transactions::PostgresTransaction::new(tx_sqlx);

    // 3. Save
    repo.create_account(&account, &mut wrapped_tx).await.unwrap();

    // 4. Rollback
    wrapped_tx.into_inner().rollback().await.unwrap();

    // 5. Vérification : le compte ne doit pas exister
    let found = repo.find_account_by_id(account.id(), None).await.unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn test_unique_constraints_violation() {
    let (pool, _c) = crate::common::setup_postgres_test_db().await;
    let repo = PostgresAccountRepository::new(pool.clone());

    let original = Account::builder(
        AccountId::new(),
        RegionCode::try_new("eu").unwrap(),
        Username::try_new("original").unwrap(),
        Email::try_new("original@test.com").unwrap(),
        ExternalId::try_new(Uuid::now_v7().to_string()).unwrap(),
    ).build();

    let mut tx = pool.begin().await.unwrap();
    let mut wrapped_tx = shared_kernel::infrastructure::postgres::transactions::PostgresTransaction::new(tx);
    repo.create_account(&original, &mut wrapped_tx).await.unwrap();
    wrapped_tx.into_inner().commit().await.unwrap();

    // TENTATIVE 1 : Même Username
    let duplicate_username = Account::builder(
        AccountId::new(),
        RegionCode::try_new("eu").unwrap(),
        Username::try_new("original").unwrap(), // CONFLIT
        Email::try_new("other@test.com").unwrap(),
        ExternalId::try_new(Uuid::now_v7().to_string()).unwrap(),
    ).build();

    let mut tx = pool.begin().await.unwrap();
    let mut wrapped_tx = shared_kernel::infrastructure::postgres::transactions::PostgresTransaction::new(tx);
    let result = repo.create_account(&duplicate_username, &mut wrapped_tx).await;

    assert!(result.is_err(), "Should have failed due to duplicate username");
}

#[tokio::test]
async fn test_patch_account_integrity() {
    let (pool, _c) = crate::common::setup_postgres_test_db().await;
    let repo = PostgresAccountRepository::new(pool.clone());

    let account = Account::builder(
        AccountId::new(),
        RegionCode::try_new("eu").unwrap(),
        Username::try_new("patch_me").unwrap(),
        Email::try_new("old@test.com").unwrap(),
        ExternalId::try_new(Uuid::now_v7().to_string()).unwrap(),
    ).build();

    // Initial save
    let mut tx = pool.begin().await.unwrap();
    let mut wrapped_tx = shared_kernel::infrastructure::postgres::transactions::PostgresTransaction::new(tx);
    repo.create_account(&account, &mut wrapped_tx).await.unwrap();
    wrapped_tx.into_inner().commit().await.unwrap();

    // Action : On patch uniquement l'email
    let params = account::domain::params::PatchUserParams {
        email: Some(Email::try_new("new@test.com").unwrap()),
        ..Default::default()
    };

    let mut tx = pool.begin().await.unwrap();
    let mut wrapped_tx = shared_kernel::infrastructure::postgres::transactions::PostgresTransaction::new(tx);
    repo.patch_account_by_id(account.id(), params, &mut wrapped_tx).await.unwrap();
    wrapped_tx.into_inner().commit().await.unwrap();

    // Vérification : L'email a changé, mais le username est resté intact
    let updated = repo.find_account_by_id(account.id(), None).await.unwrap().unwrap();
    assert_eq!(updated.email().as_str(), "new@test.com");
    assert_eq!(updated.username().as_str(), "patch_me");
}

#[tokio::test]
async fn test_upsert_idempotency() {
    let (pool, _c) = crate::common::setup_postgres_test_db().await;
    let repo = PostgresAccountRepository::new(pool.clone());

    let mut account = Account::builder(
        AccountId::new(),
        RegionCode::try_new("eu").unwrap(),
        Username::try_new("upsert_user").unwrap(),
        Email::try_new("upsert@test.com").unwrap(),
        ExternalId::try_new(Uuid::now_v7().to_string()).unwrap(),
    ).build();

    // Premier passage (Insert)
    repo.save(&account, None).await.expect("First save failed");

    // Modification locale
    account.deactivate(); // On suppose que cette méthode change le state en 'deactivated'

    // Deuxième passage (Update via ON CONFLICT)
    repo.save(&account, None).await.expect("Second save failed");

    let final_acc = repo.find_account_by_id(account.id(), None).await.unwrap().unwrap();
    assert_eq!(final_acc.state(), &AccountState::Deactivated);
}