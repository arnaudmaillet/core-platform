use account::domain::entities::AccountSettings;
use account::domain::repositories::{AccountRepository, AccountSettingsRepository};
use account::infrastructure::postgres::repositories::PostgresAccountSettingsRepository;
use shared_kernel::domain::value_objects::{AccountId, PushToken, Timezone, RegionCode};
use shared_kernel::infrastructure::postgres::transactions::PostgresTransaction;


async fn get_repo() -> (
    PostgresAccountSettingsRepository,
    testcontainers::ContainerAsync<testcontainers_modules::postgres::Postgres>,
) {
    let (pool, container) = crate::common::setup_postgres_test_db().await;
    (PostgresAccountSettingsRepository::new(pool), container)
}

#[tokio::test]
async fn test_settings_lifecycle_and_upsert() {
    let (pool, _c) = crate::common::setup_postgres_test_db().await;
    let repo = PostgresAccountSettingsRepository::new(pool.clone());
    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu".to_string()).unwrap();

    // 1. Création initiale (Upsert - Insert)
    let settings = AccountSettings::builder(account_id.clone(), region.clone())
        .with_timezone(Timezone::try_new("Europe/Paris".to_string()).unwrap())
        .build();

    repo.save(&settings, None).await.expect("Should save initial settings");

    // 2. Vérification
    let found = repo.find_by_account_id(&account_id, None).await.unwrap().expect("Should find settings");
    assert_eq!(found.timezone().as_str(), "Europe/Paris");

    // 3. Mise à jour via save (Upsert - Update)
    let mut updated_settings = found;
    updated_settings.update_timezone(Timezone::try_new("UTC".to_string()).unwrap());

    repo.save(&updated_settings, None).await.expect("Should update settings");

    let final_check = repo.find_by_account_id(&account_id, None).await.unwrap().unwrap();
    assert_eq!(final_check.timezone().as_str(), "UTC");
}

#[tokio::test]
async fn test_push_tokens_atomic_operations() {
    let (pool, _c) = crate::common::setup_postgres_test_db().await;
    let repo = PostgresAccountSettingsRepository::new(pool.clone());
    let account_id = AccountId::new();
    let region = RegionCode::try_new("us".to_string()).unwrap();

    // On crée les settings de base
    let settings = AccountSettings::builder(account_id.clone(), region).build();
    repo.save(&settings, None).await.unwrap();

    let token_1 = PushToken::try_new("token_alpha".to_string()).unwrap();
    let token_2 = PushToken::try_new("token_beta".to_string()).unwrap();

    // 1. Ajout du premier token
    repo.add_push_token(&account_id, &token_1, None).await.unwrap();

    // 2. Ajout du second token (doit s'ajouter à la liste, pas remplacer)
    repo.add_push_token(&account_id, &token_2, None).await.unwrap();

    // 3. Vérification de la liste
    let found = repo.find_by_account_id(&account_id, None).await.unwrap().unwrap();
    assert_eq!(found.push_tokens().len(), 2);
    assert!(found.push_tokens().contains(&token_1));
    assert!(found.push_tokens().contains(&token_2));

    // 4. Suppression d'un token spécifique
    repo.remove_push_token(&account_id, &token_1, None).await.unwrap();

    let after_remove = repo.find_by_account_id(&account_id, None).await.unwrap().unwrap();
    assert_eq!(after_remove.push_tokens().len(), 1);
    assert!(!after_remove.push_tokens().contains(&token_1));
    assert!(after_remove.push_tokens().contains(&token_2));
}

#[tokio::test]
async fn test_timezone_direct_update() {
    let (pool, _c) = crate::common::setup_postgres_test_db().await;
    let repo = PostgresAccountSettingsRepository::new(pool.clone());
    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();

    repo.save(&AccountSettings::builder(account_id.clone(), region).build(), None).await.unwrap();

    let new_tz = Timezone::try_new("Asia/Tokyo".to_string()).unwrap();
    repo.update_timezone(&account_id, &new_tz, None).await.unwrap();

    let found = repo.find_by_account_id(&account_id, None).await.unwrap().unwrap();
    assert_eq!(found.timezone().as_str(), "Asia/Tokyo");
}

#[tokio::test]
async fn test_settings_transactional_integrity() {
    let (pool, _c) = crate::common::setup_postgres_test_db().await;
    let repo = PostgresAccountSettingsRepository::new(pool.clone());
    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();

    // Démarrage transaction
    let tx_sqlx = pool.begin().await.unwrap();
    let mut tx = PostgresTransaction::new(tx_sqlx);

    let settings = AccountSettings::builder(account_id.clone(), region).build();

    // On sauve dans la transaction
    repo.save(&settings, Some(&mut tx)).await.unwrap();

    // On rollback
    tx.into_inner().rollback().await.unwrap();

    // Vérification : ne doit pas exister
    let found = repo.find_by_account_id(&account_id, None).await.unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn test_delete_settings() {
    let (pool, _c) = crate::common::setup_postgres_test_db().await;
    let repo = PostgresAccountSettingsRepository::new(pool.clone());
    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();

    repo.save(&AccountSettings::builder(account_id.clone(), region).build(), None).await.unwrap();

    repo.delete_for_user(&account_id, None).await.unwrap();

    let found = repo.find_by_account_id(&account_id, None).await.unwrap();
    assert!(found.is_none());
}