// crates/profile/tests/infrastructure/profile_repository_it.rs

use profile::domain::entities::Profile;
use profile::domain::repositories::ProfileIdentityRepository;
use profile::domain::value_objects::{Bio, DisplayName, Handle, ProfileId};
use profile::infrastructure::postgres::repositories::PostgresIdentityRepository;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Url};
use shared_kernel::errors::DomainError;

/// Helper pour centraliser l'init du repo par test
async fn get_repo() -> (
    PostgresIdentityRepository,
    testcontainers::ContainerAsync<testcontainers_modules::postgres::Postgres>,
) {
    let (pool, container) = crate::common::setup_postgres_test_db().await;
    (PostgresIdentityRepository::new(pool), container)
}

#[tokio::test]
async fn test_profile_lifecycle() {
    let (repo, _c) = get_repo().await;
    let owner_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();

    // 1. Création initiale (Utilisation du builder avec Handle)
    let profile = Profile::builder(
        owner_id.clone(),
        region.clone(),
        DisplayName::from_raw("Alice"),
        Handle::try_new("alice_dev").unwrap(),
    )
        .build();

    let profile_id = profile.id().clone();

    repo.save(&profile, None)
        .await
        .expect("Initial save failed");

    // 2. Vérification de l'existence par Handle
    let exists = repo
        .exists_by_handle(profile.handle(), &region)
        .await
        .unwrap();
    assert!(exists);

    // 3. Mise à jour (Versioning & Persistance)
    let mut profile_to_update = repo
        .fetch(&profile_id, &region)
        .await
        .unwrap()
        .unwrap();

    profile_to_update.update_display_name(&region, DisplayName::from_raw("Alice Updated")).unwrap();
    repo.save(&profile_to_update, None).await.expect("Update failed");

    // 4. Validation des données persistées
    let fetched = repo
        .fetch(&profile_id, &region)
        .await
        .unwrap()
        .expect("Should find profile");

    assert_eq!(fetched.display_name().as_str(), "Alice Updated");
    assert_eq!(fetched.version(), 2); // Incrémenté par la logique domaine

    // 5. Suppression
    repo.delete(&profile_id, &region)
        .await
        .expect("Delete failed");
    let after_delete = repo.fetch(&profile_id, &region).await.unwrap();
    assert!(after_delete.is_none());
}

#[tokio::test]
async fn test_fetch_by_handle_not_found() {
    let (repo, _c) = get_repo().await;
    let region = RegionCode::try_new("eu").unwrap();
    let handle = Handle::try_new("unknown_user").unwrap();

    let result = repo.fetch_by_handle(&handle, &region).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_concurrency_conflict_real_scenario() {
    let (repo, _c) = get_repo().await;
    let region = RegionCode::try_new("eu").unwrap();
    let profile_id = ProfileId::new();

    let profile = Profile::builder(
        AccountId::new(),
        region.clone(),
        DisplayName::from_raw("Concurrency Test"),
        Handle::try_new("concurrent_user").unwrap(),
    )
        .build();

    // Premier save (v1)
    repo.save(&profile, None).await.unwrap();

    // Chargement de deux instances concurrentes (v1)
    let mut instance_a = repo.fetch(profile.id(), &region).await.unwrap().unwrap();
    let mut instance_b = repo.fetch(profile.id(), &region).await.unwrap().unwrap();

    // A gagne et passe en v2
    instance_a.update_display_name(&region, DisplayName::from_raw("Winner")).unwrap();
    repo.save(&instance_a, None).await.unwrap();

    // B essaie de save sa v1 alors que la DB est en v2
    instance_b.update_display_name(&region, DisplayName::from_raw("Loser")).unwrap();
    let result = repo.save(&instance_b, None).await;

    assert!(matches!(
        result,
        Err(DomainError::ConcurrencyConflict { .. })
    ));
}

#[tokio::test]
async fn test_unique_handle_constraint() {
    let (repo, _c) = get_repo().await;
    let region = RegionCode::try_new("eu").unwrap();
    let handle = Handle::try_new("unique_bob").unwrap();

    let bob1 = Profile::builder(AccountId::new(), region.clone(), DisplayName::from_raw("Bob 1"), handle.clone()).build();
    repo.save(&bob1, None).await.expect("First save should work");

    let bob2 = Profile::builder(AccountId::new(), region.clone(), DisplayName::from_raw("Bob 2"), handle).build();
    let result = repo.save(&bob2, None).await;

    assert!(result.is_err(), "Should have failed due to duplicate handle in same region");
}

#[tokio::test]
async fn test_partial_update_integrity() {
    let (repo, _c) = get_repo().await;
    let region = RegionCode::try_new("eu").unwrap();

    // 1. Création avec bio et avatar
    let mut profile = Profile::builder(
        AccountId::new(),
        region.clone(),
        DisplayName::from_raw("Alice"),
        Handle::try_new("alice").unwrap(),
    ).build();

    profile.update_bio(&region, Some(Bio::try_new("Ma bio initiale").unwrap())).unwrap();
    profile.update_avatar(&region, Url::try_new("https://avatar.com/1").unwrap()).unwrap();
    repo.save(&profile, None).await.unwrap();

    // 2. Update uniquement du display_name
    let mut to_update = repo.fetch(profile.id(), &region).await.unwrap().unwrap();
    to_update.update_display_name(&region, DisplayName::from_raw("Alice Nouvelle")).unwrap();
    repo.save(&to_update, None).await.unwrap();

    // 3. Vérification de l'intégrité (pas de perte des champs non modifiés)
    let final_p = repo.fetch(profile.id(), &region).await.unwrap().unwrap();
    assert_eq!(final_p.display_name().as_str(), "Alice Nouvelle");
    assert_eq!(final_p.bio().as_ref().map(|b| b.as_str()), Some("Ma bio initiale"));
    assert_eq!(final_p.avatar_url().as_ref().map(|b| b.as_str()), Some("https://avatar.com/1"));
}

#[tokio::test]
async fn test_transaction_rollback_logic() {
    let (pool, _c) = crate::common::setup_postgres_test_db().await;
    let repo = PostgresIdentityRepository::new(pool.clone());
    let region = RegionCode::try_new("eu").unwrap();

    let tx_sqlx = pool.begin().await.unwrap();
    let mut wrapped_tx = shared_kernel::infrastructure::postgres::transactions::PostgresTransaction::new(tx_sqlx);

    let profile = Profile::builder(
        AccountId::new(),
        region.clone(),
        DisplayName::from_raw("Ghost"),
        Handle::try_new("ghost").unwrap(),
    ).build();

    let pid = profile.id().clone();
    repo.save(&profile, Some(&mut wrapped_tx)).await.unwrap();

    // Annulation explicite
    wrapped_tx.into_inner().rollback().await.unwrap();

    let found = repo.fetch(&pid, &region).await.unwrap();
    assert!(found.is_none(), "Le profil ne devrait pas exister après un rollback");
}