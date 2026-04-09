// crates/account/tests/repositories/account_settings_repository_it.rs

use account::domain::account::entities::{AccountSettings};
use account::domain::preferences::models::{AppearancePreferences, ThemeMode};
use account::domain::repositories::AccountSettingsRepository;
use account::infrastructure::postgres::repositories::PostgresAccountSettingsRepository;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::value_objects::{AccountId, PushToken, Timezone};
use shared_kernel::domain::repositories::CacheRepository;
use shared_kernel::infrastructure::postgres::transactions::PostgresTransaction;
use shared_kernel::infrastructure::postgres::utils::PostgresTestContext;
use shared_kernel::infrastructure::redis::utils::RedisTestContext;

/// Helper pour instancier le repo et la pool
async fn get_test_context() -> (PostgresAccountSettingsRepository, PostgresTestContext, RedisTestContext) {
    let pg_ctx = PostgresTestContext::builder()
        .with_migrations(&["./migrations/postgres"])
        .build()
        .await;

    let redis_ctx = RedisTestContext::builder().build().await;

    let repo = PostgresAccountSettingsRepository::new(
        pg_ctx.pool().clone(),
        redis_ctx.repository()
    );
    
    (repo, pg_ctx, redis_ctx)
}

#[tokio::test]
async fn test_settings_cache_invalidation_lifecycle() {
    let (repo, _pg_ctx, redis_ctx) = get_test_context().await;
    let cache = redis_ctx.repository();
    let account_id = AccountId::new();
    let cache_key = format!("account:settings:{}", account_id.clone());

    let settings = AccountSettings::builder(account_id.clone()).build();
    repo.save(&settings, None, None).await.unwrap();

    // 1. Premier Fetch -> Remplissage Cache
    let _ = repo.fetch_by_account_id(&account_id, None).await.unwrap().unwrap();
    assert!(cache.exists(&cache_key).await.unwrap(), "Settings should be cached");

    // 2. Ajout d'un push token -> Doit INVALIDER le cache
    let token = PushToken::try_new("new_token").unwrap();
    repo.add_push_token(&account_id, &token, None).await.unwrap();

    assert!(!cache.exists(&cache_key).await.unwrap(), "Cache must be cleared after atomic SQL update");

    // 3. Second Fetch -> Rechargement
    let reloaded = repo.fetch_by_account_id(&account_id, None).await.unwrap().unwrap();
    assert!(reloaded.push_tokens().contains(&token));
    assert!(cache.exists(&cache_key).await.unwrap(), "Cache should be refilled");
}

#[tokio::test]
async fn test_settings_transaction_skips_cache() {
    let (repo, pg_ctx, redis_ctx) = get_test_context().await;
    let cache = redis_ctx.repository();
    let account_id = AccountId::new();
    let cache_key = format!("account:settings:{}", account_id.clone());

    let settings = AccountSettings::builder(account_id.clone()).build();
    repo.save(&settings, None, None).await.unwrap();

    // DANS une transaction, on ne doit pas écrire dans le cache (Dirty Write protection)
    let tx_sqlx = pg_ctx.pool().begin().await.unwrap();
    let mut tx = PostgresTransaction::new(tx_sqlx);

    let _ = repo.fetch_by_account_id(&account_id, Some(&mut tx)).await.unwrap();
    
    assert!(!cache.exists(&cache_key).await.unwrap(), "Should not cache settings during a transaction");
}

#[tokio::test]
async fn test_settings_lifecycle_and_upsert() {
    let (repo, _pg_ctx, _redis_ctx) = get_test_context().await;
    let account_id = AccountId::new();

    let appearance = AppearancePreferences::builder()
        .with_theme(ThemeMode::Light)
        .with_high_contrast(true)
        .build();
    let settings = AccountSettings::builder(account_id.clone())
        .with_timezone(Timezone::try_new("Europe/Paris").unwrap())
        .with_appearance(appearance.clone())
        .build();

    repo.save(&settings, None, None).await.expect("Should save initial settings");

    // 2. Vérification du fetch et du contenu du JSONB
    let found = repo.fetch_by_account_id(&account_id, None).await.unwrap().expect("Should find settings");
    assert_eq!(found.timezone().as_str(), "Europe/Paris");
    assert_eq!(found.preferences().appearance(), &appearance); // Vérifie le bloc JSONB
    assert_eq!(found.version(), 1);

    // 3. Mise à jour via une méthode granulaire (v1 -> v2)
    let mut updated_settings = found.clone();
    updated_settings.update_timezone(Timezone::try_new("UTC").unwrap()).unwrap();

    // On passe 'found' comme original pour le lock optimiste (version check)
    repo.save(&updated_settings, Some(&found), None).await.expect("Should update settings");

    let final_check = repo.fetch_by_account_id(&account_id, None).await.unwrap().unwrap();
    assert_eq!(final_check.timezone().as_str(), "UTC");
    assert_eq!(final_check.version(), 2);
}


#[tokio::test]
async fn test_update_preferences_persistence() {
    let (repo, _pg_ctx, _redis_ctx) = get_test_context().await;
    let account_id = AccountId::new();

    let settings = AccountSettings::builder(account_id.clone()).build();
    repo.save(&settings, None, None).await.unwrap();

    let found = repo.fetch_by_account_id(&account_id, None).await.unwrap().unwrap();
    let mut updated_settings = found.clone();

    // Modification du bloc Appearance dans le domaine
    let new_appearance = AppearancePreferences::builder()
        .with_theme(ThemeMode::Dark)
        .with_high_contrast(false)
        .build();
    updated_settings.update_appearance_preferences(new_appearance.clone()).unwrap();

    repo.save(&updated_settings, Some(&found), None).await.unwrap();

    // Vérification que le JSONB a bien été mis à jour en base
    let final_check = repo.fetch_by_account_id(&account_id, None).await.unwrap().unwrap();
    assert_eq!(final_check.preferences().appearance(), &new_appearance);
}

#[tokio::test]
async fn test_push_tokens_atomic_operations() {
    let (repo, _pg_ctx, _redis_ctx) = get_test_context().await;
    let account_id = AccountId::new();

    let settings = AccountSettings::builder(account_id.clone()).build();
    repo.save(&settings, None, None).await.unwrap();

    let token_1 = PushToken::try_new("token_alpha").unwrap();
    let token_2 = PushToken::try_new("token_beta").unwrap();

    // 1. Ajout atomique (SQL direct)
    repo.add_push_token(&account_id, &token_1, None).await.unwrap();
    repo.add_push_token(&account_id, &token_2, None).await.unwrap();

    // 2. Vérification
    let found = repo.fetch_by_account_id(&account_id, None).await.unwrap().unwrap();
    assert_eq!(found.push_tokens().len(), 2);
    assert!(found.push_tokens().contains(&token_1));

    // 3. Suppression
    repo.remove_push_token(&account_id, &token_1, None).await.unwrap();

    let after_remove = repo.fetch_by_account_id(&account_id, None).await.unwrap().unwrap();
    assert_eq!(after_remove.push_tokens().len(), 1);
    assert!(!after_remove.push_tokens().contains(&token_1));
}

#[tokio::test]
async fn test_settings_transactional_integrity() {
    let (repo, pg_ctx, _redis_ctx) = get_test_context().await;
    let account_id = AccountId::new();

    let tx_sqlx = pg_ctx.pool().begin().await.unwrap();
    let mut tx = PostgresTransaction::new(tx_sqlx);

    let settings = AccountSettings::builder(account_id.clone()).build();
    // Utilisation de Some(&mut tx)
    repo.save(&settings, None, Some(&mut tx)).await.unwrap();

    // Rollback explicite
    tx.into_inner().rollback().await.unwrap();

    let found = repo.fetch_by_account_id(&account_id, None).await.unwrap();
    assert!(found.is_none(), "Settings should not exist after rollback");
}

#[tokio::test]
async fn test_settings_concurrency_conflict() {
    let (repo, _pg_ctx, _redis_ctx) = get_test_context().await;
    let account_id = AccountId::new();

    // 1. Initialisation (v1)
    let settings = AccountSettings::builder(account_id.clone()).build();
    repo.save(&settings, None, None).await.unwrap();

    // 2. Deux clients lisent la v1
    let client_a_found = repo.fetch_by_account_id(&account_id, None).await.unwrap().unwrap();
    let client_b_found = repo.fetch_by_account_id(&account_id, None).await.unwrap().unwrap();

    // 3. Client A gagne (v1 -> v2)
    let mut client_a_modified = client_a_found.clone();
    client_a_modified.update_timezone(Timezone::try_new("Europe/Berlin").unwrap()).unwrap();
    repo.save(&client_a_modified, Some(&client_a_found), None).await.expect("A should succeed");

    // 4. Client B échoue (tente v1 -> v2 alors que la DB est en v2)
    let mut client_b_modified = client_b_found.clone();
    client_b_modified.update_timezone(Timezone::try_new("Europe/London").unwrap()).unwrap();
    
    let result = repo.save(&client_b_modified, Some(&client_b_found), None).await;

    assert!(result.is_err(), "B should fail due to optimistic locking conflict (OCC)");
    assert!(matches!(
        result.unwrap_err(),
        shared_kernel::errors::DomainError::ConcurrencyConflict { .. }
    ));
}

#[tokio::test]
async fn test_push_token_idempotency_it() {
    let (repo, _pg_ctx, _redis_ctx) = get_test_context().await;
    let account_id = AccountId::new();
    let token = PushToken::try_new("unique_token").unwrap();

    repo.save(&AccountSettings::builder(account_id.clone()).build(), None, None).await.unwrap();

    // Ajout double du même token
    repo.add_push_token(&account_id, &token, None).await.unwrap();
    repo.add_push_token(&account_id, &token, None).await.unwrap();

    let found = repo.fetch_by_account_id(&account_id, None).await.unwrap().expect("Should exist");
    assert_eq!(found.push_tokens().len(), 1, "Token should not be duplicated in DB via SQL atomic query");
}