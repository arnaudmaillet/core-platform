// crates/account/tests/repositories/account_settings_repository_it.rs

use account::domain::entities::AccountSettings;
use account::domain::repositories::AccountSettingsRepository;
use account::infrastructure::postgres::repositories::PostgresAccountSettingsRepository;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::value_objects::{AccountId, PushToken, Timezone, RegionCode};
use shared_kernel::infrastructure::postgres::transactions::PostgresTransaction;
use shared_kernel::infrastructure::postgres::utils::PostgresTestContext;

/// Helper pour instancier le repo et la pool
async fn get_test_context() -> (PostgresAccountSettingsRepository, PostgresTestContext) {
    let ctx = PostgresTestContext::builder()
        .with_migrations(&["./migrations/postgres"])
        .build()
        .await;

    let repo = PostgresAccountSettingsRepository::new(ctx.pool());
    (repo, ctx)
}

#[tokio::test]
async fn test_settings_lifecycle_and_upsert() {
    let (repo, ctx) = get_test_context().await;
    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();

    // 1. Création initiale (v1)
    let settings = AccountSettings::builder(account_id.clone(), region.clone())
        .with_timezone(Timezone::try_new("Europe/Paris").unwrap())
        .build();

    repo.save(&settings, None).await.expect("Should save initial settings");

    // 2. Vérification
    let found = repo.find_by_account_id(&account_id, None).await.unwrap().expect("Should find settings");
    assert_eq!(found.timezone().as_str(), "Europe/Paris");
    assert_eq!(found.version(), 1);

    // 3. Mise à jour via le domaine (v1 -> v2)
    let mut updated_settings = found;
    updated_settings.update_timezone(&region, Timezone::try_new("UTC").unwrap()).unwrap();

    repo.save(&updated_settings, None).await.expect("Should update settings");

    let final_check = repo.find_by_account_id(&account_id, None).await.unwrap().unwrap();
    assert_eq!(final_check.timezone().as_str(), "UTC");
    assert_eq!(final_check.version(), 2);
}

#[tokio::test]
async fn test_push_tokens_atomic_operations() {
    let (repo, _ctx) = get_test_context().await;
    let account_id = AccountId::new();
    let region = RegionCode::try_new("us").unwrap();

    let settings = AccountSettings::builder(account_id.clone(), region.clone()).build();
    repo.save(&settings, None).await.unwrap();

    let token_1 = PushToken::try_new("token_alpha").unwrap();
    let token_2 = PushToken::try_new("token_beta").unwrap();

    // 1. Ajout atomique
    repo.add_push_token(&account_id, &token_1, None).await.unwrap();
    repo.add_push_token(&account_id, &token_2, None).await.unwrap();

    // 2. Vérification
    let found = repo.find_by_account_id(&account_id, None).await.unwrap().unwrap();
    assert_eq!(found.push_tokens().len(), 2);
    assert!(found.push_tokens().contains(&token_1));

    // 3. Suppression
    repo.remove_push_token(&account_id, &token_1, None).await.unwrap();

    let after_remove = repo.find_by_account_id(&account_id, None).await.unwrap().unwrap();
    assert_eq!(after_remove.push_tokens().len(), 1);
    assert!(!after_remove.push_tokens().contains(&token_1));
}

#[tokio::test]
async fn test_settings_transactional_integrity() {
    let (repo, ctx) = get_test_context().await;
    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();

    let tx_sqlx = ctx.pool().begin().await.unwrap();
    let mut tx = PostgresTransaction::new(tx_sqlx);

    let settings = AccountSettings::builder(account_id.clone(), region).build();
    repo.save(&settings, Some(&mut tx)).await.unwrap();

    // Rollback explicite
    tx.into_inner().rollback().await.unwrap();

    let found = repo.find_by_account_id(&account_id, None).await.unwrap();
    assert!(found.is_none(), "Settings should not exist after rollback");
}

#[tokio::test]
async fn test_settings_concurrency_conflict() {
    let (repo, _ctx) = get_test_context().await;
    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();

    // 1. Initialisation (v1)
    let settings = AccountSettings::builder(account_id.clone(), region.clone()).build();
    repo.save(&settings, None).await.unwrap();

    // 2. Deux clients lisent la v1
    let mut client_a = repo.find_by_account_id(&account_id, None).await.unwrap().unwrap();
    let mut client_b = repo.find_by_account_id(&account_id, None).await.unwrap().unwrap();

    // 3. Client A gagne (v1 -> v2)
    client_a.update_timezone(&region, Timezone::try_new("Europe/Berlin").unwrap()).unwrap();
    repo.save(&client_a, None).await.expect("A should succeed");

    // 4. Client B échoue
    client_b.update_timezone(&region, Timezone::try_new("Europe/London").unwrap()).unwrap();
    let result = repo.save(&client_b, None).await;

    assert!(result.is_err(), "B should fail due to optimistic locking conflict (OCC)");
}

#[tokio::test]
async fn test_push_token_idempotency_it() {
    let (repo, _ctx) = get_test_context().await;
    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();
    let token = PushToken::try_new("unique_token").unwrap();

    repo.save(&AccountSettings::builder(account_id.clone(), region).build(), None).await.unwrap();

    // Ajout double du même token
    repo.add_push_token(&account_id, &token, None).await.unwrap();
    repo.add_push_token(&account_id, &token, None).await.unwrap();

    let found = repo.find_by_account_id(&account_id, None).await.unwrap().expect("Should exist");
    assert_eq!(found.push_tokens().len(), 1, "Token should not be duplicated in DB");
}

#[tokio::test]
async fn test_settings_region_mismatch_security() {
    let (repo, _ctx) = get_test_context().await;

    let account_id = AccountId::new();
    let region_eu = RegionCode::try_new("eu").unwrap();
    let region_us = RegionCode::try_new("us").unwrap();

    let mut settings = AccountSettings::builder(account_id, region_eu).build();

    // Tentative de modification avec la mauvaise région (US au lieu de EU)
    // La logique de garde du domaine doit bloquer l'appel avant même d'atteindre le repository
    let result = settings.update_timezone(&region_us, Timezone::try_new("UTC").unwrap());

    // Assertions
    assert!(result.is_err(), "Le domaine devrait interdire la modification d'un shard différent");
    assert!(matches!(
        result.unwrap_err(),
        shared_kernel::errors::DomainError::Forbidden { .. }
    ));
}