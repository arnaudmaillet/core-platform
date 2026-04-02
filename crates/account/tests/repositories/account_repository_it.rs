// crates/account/tests/repositories/account_repository_it.rs


use account::domain::entities::account::Account;
use account::domain::repositories::AccountRepository;
use account::infrastructure::postgres::repositories::PostgresAccountRepository;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use account::domain::value_objects::{Email, ExternalId, AccountState};
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::infrastructure::postgres::utils::PostgresTestContext;
use shared_kernel::infrastructure::postgres::transactions::PostgresTransaction;

/// Helper pour instancier le repo et la DB de test
async fn get_test_context() -> (PostgresAccountRepository, PostgresTestContext) {
    let ctx = PostgresTestContext::builder()
        .with_migrations(&["./migrations/postgres"])
        .build()
        .await;

    let repo = PostgresAccountRepository::new(ctx.pool().clone());
    (repo, ctx)
}

#[tokio::test]
async fn test_account_lifecycle_full() {
    let (repo, ctx) = get_test_context().await;
    let pool = ctx.pool();
    let region = RegionCode::try_new("eu").unwrap();
    let account_id = AccountId::new();

    // 1. Création initiale (Version 1)
    let account = Account::builder(
        account_id.clone(),
        region.clone(),
        Email::try_new("sonny@rust.com").unwrap(),
        ExternalId::from_raw("ext_lifecycle"),
    ).build();

    // Utilisation du nouveau système de transaction
    let tx_sqlx = pool.begin().await.unwrap();
    let mut wrapped_tx = PostgresTransaction::new(tx_sqlx);

    // Note: save() gère l'insert si original est None
    repo.save(&account, None, Some(&mut wrapped_tx)).await.expect("Initial creation failed");
    wrapped_tx.into_inner().commit().await.unwrap();

    // 2. Vérification fetch_by_id
    let found = repo.fetch_by_id(&account_id, None).await.unwrap().expect("Should find account");
    assert_eq!(found.version(), 1);

    // 3. Update (v1 -> v2)
    let mut to_update = found.clone();
    to_update.deactivate(&region).expect("Deactivation failed");
    
    // On passe 'found' comme original pour activer le verrouillage optimiste
    repo.save(&to_update, Some(&found), None).await.expect("Save v2 failed");

    let updated = repo.fetch_by_id(&account_id, None).await.unwrap().unwrap();
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
        Email::try_new("ghost@void.com").unwrap(),
        ExternalId::from_raw("ext_ghost"),
    ).build();

    let tx_sqlx = ctx.pool().begin().await.unwrap();
    let mut wrapped_tx = PostgresTransaction::new(tx_sqlx);

    repo.save(&account, None, Some(&mut wrapped_tx)).await.unwrap();
    wrapped_tx.into_inner().rollback().await.unwrap();

    let found = repo.fetch_by_id(&account_id, None).await.unwrap();
    assert!(found.is_none(), "Account should not exist after rollback");
}

#[tokio::test]
async fn test_unique_constraints_violation() {
    let (repo, _ctx) = get_test_context().await;
    let region = RegionCode::try_new("eu").unwrap();
    let email_str = "duplicate@test.com";

    let original = Account::builder(
        AccountId::new(),
        region.clone(),
        Email::try_new(email_str).unwrap(),
        ExternalId::from_raw("ext_1"),
    ).build();

    repo.save(&original, None, None).await.unwrap();

    let duplicate = Account::builder(
        AccountId::new(),
        region,
        Email::try_new(email_str).unwrap(), // Même email
        ExternalId::from_raw("ext_2"),
    ).build();

    let result = repo.save(&duplicate, None, None).await;
    assert!(result.is_err(), "Duplicate email should trigger unique constraint");
}

#[tokio::test]
async fn test_account_concurrency_conflict_it() {
    let (repo, _ctx) = get_test_context().await;
    let region = RegionCode::try_new("eu").unwrap();
    let account_id = AccountId::new();

    let account = Account::builder(
        account_id.clone(),
        region.clone(),
        Email::try_new("concurrent@test.com").unwrap(),
        ExternalId::from_raw("ext_concurrent"),
    ).build();
    repo.save(&account, None, None).await.unwrap();

    // On simule deux clients qui chargent la même version (v1)
    let client_a_found = repo.fetch_by_id(&account_id, None).await.unwrap().unwrap();
    let client_b_found = repo.fetch_by_id(&account_id, None).await.unwrap().unwrap();

    // Client A sauvegarde en premier (v1 -> v2)
    let mut client_a_modified = client_a_found.clone();
    client_a_modified.deactivate(&region).unwrap();
    repo.save(&client_a_modified, Some(&client_a_found), None).await.expect("Client A wins");

    // Client B essaie de sauvegarder sa version v1 (v1 -> v2) MAIS la DB est déjà en v2
    let mut client_b_modified = client_b_found.clone();
    client_b_modified.suspend(&region, "Late update".into()).unwrap();
    
    let result = repo.save(&client_b_modified, Some(&client_b_found), None).await;

    // L'erreur doit être un ConcurrencyConflict
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        shared_kernel::errors::DomainError::ConcurrencyConflict { .. }
    ));
}

#[tokio::test]
async fn test_account_lookups_and_resolutions() {
    let (repo, _ctx) = get_test_context().await;
    let email = Email::try_new("lookup@test.com").unwrap();
    let ext_id = ExternalId::from_raw("ext_lookup_123");

    let account = Account::builder(
        AccountId::new(),
        RegionCode::try_new("eu").unwrap(),
        email.clone(),
        ext_id.clone(),
    ).build();

    repo.save(&account, None, None).await.unwrap();

    // Test des méthodes de vérification
    assert!(repo.exists_by_email(&email).await.unwrap());
    assert!(repo.exists_by_external_id(&ext_id).await.unwrap());

    // Test des résolutions d'ID
    let id_from_email = repo.resolve_id_from_email(&email).await.unwrap();
    assert_eq!(id_from_email.unwrap(), *account.id());

    let id_from_ext = repo.resolve_id_from_external_id(&ext_id).await.unwrap();
    assert_eq!(id_from_ext.unwrap(), *account.id());
}

#[tokio::test]
async fn test_account_security_region_mismatch_it() {
    let (repo, _ctx) = get_test_context().await;

    let region_eu = RegionCode::try_new("eu").unwrap();
    let region_us = RegionCode::try_new("us").unwrap();
    let account_id = AccountId::new();

    // 1. Arrange : Création d'un compte rattaché à la région EU
    let mut account = Account::builder(
        account_id,
        region_eu.clone(),
        Email::try_new("security_test@test.com").unwrap(),
        ExternalId::from_raw("ext_sec_777"),
    ).build();

    // Persistance initiale
    repo.save(&account, None, None).await.expect("Initial save failed");

    // 2. Act : Tentative de mutation métier en fournissant une région US
    // Le domaine doit comparer la région fournie dans l'appel (region_us) 
    // avec sa région interne (region_eu).
    let result = account.deactivate(&region_us);

    // 3. Assert : Vérification que le garde-fou du Domaine a fonctionné
    assert!(result.is_err(), "Le domaine devrait interdire une action sur une région mismatch");
    
    if let Err(shared_kernel::errors::DomainError::Forbidden { reason }) = result {
        assert!(reason.contains("region"), "L'erreur devrait mentionner le conflit de région");
    } else {
        panic!("Devrait retourner une DomainError::Forbidden spécifique");
    }

    // 4. Vérification en base : L'état ne doit pas avoir changé
    let found = repo.fetch_by_id(account.id(), None).await.unwrap().unwrap();
    assert_eq!(*found.state(), AccountState::Pending, "Le compte doit rester en Pending");
}