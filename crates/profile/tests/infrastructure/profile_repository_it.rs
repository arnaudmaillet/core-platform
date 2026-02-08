// crates/profile/tests/infrastructure/profile_repository_it.rs

use profile::domain::entities::Profile;
use profile::domain::repositories::ProfileIdentityRepository;
use profile::domain::value_objects::{Bio, DisplayName};
use profile::infrastructure::postgres::repositories::PostgresProfileRepository;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Url, Username};
use shared_kernel::errors::DomainError;

/// Helper pour centraliser l'init du repo par test
async fn get_repo() -> (
    PostgresProfileRepository,
    testcontainers::ContainerAsync<testcontainers_modules::postgres::Postgres>,
) {
    let (pool, container) = crate::common::setup_postgres_test_db().await;
    (PostgresProfileRepository::new(pool), container)
}

#[tokio::test]
async fn test_profile_lifecycle() {
    let (repo, _c) = get_repo().await;
    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu".to_string()).unwrap();

    // 1. Création initiale
    let mut profile = Profile::builder(
        account_id,
        region.clone(),
        DisplayName::from_raw("Alice"),
        Username::try_new("alice_dev".to_string()).unwrap(),
    )
    .build();

    repo.save(&profile, None)
        .await
        .expect("Initial save failed");

    // 2. Vérification de l'existence par Username
    let exists = repo
        .exists_by_username(&profile.username(), &region)
        .await
        .unwrap();
    assert!(exists);

    // 3. Mise à jour des champs (et test du JSONB / Versioning)
    let mut profile = repo
        .fetch(&account_id, &region)
        .await
        .unwrap()
        .unwrap();
    profile.update_display_name(&region, DisplayName::from_raw("Alice Updated")).unwrap();
    // Supposons que tu aies un champ bio ou social_links accessible
    repo.save(&profile, None).await.expect("Update failed");

    // 4. Récupération et validation des données persistées
    let fetched = repo
        .fetch(&account_id, &region)
        .await
        .unwrap()
        .expect("Should find profile");
    assert_eq!(fetched.display_name().as_str(), "Alice Updated");
    assert_eq!(fetched.version(), 2); // Le trigger ou la logique repo a incrémenté la version

    // 5. Suppression
    repo.delete(&account_id, &region)
        .await
        .expect("Delete failed");
    let after_delete = repo.fetch(&account_id, &region).await.unwrap();
    assert!(after_delete.is_none());
}

#[tokio::test]
async fn test_find_by_username_not_found() {
    let (repo, _c) = get_repo().await;
    let region = RegionCode::try_new("eu".to_string()).unwrap();
    let username = Username::try_new("unknown_user".to_string()).unwrap();

    let result = repo.fetch_by_username(&username, &region).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_concurrency_conflict_real_scenario() {
    let (repo, _c) = get_repo().await;
    let region = RegionCode::try_new("eu".to_string()).unwrap();
    let account_id = AccountId::new();

    let profile = Profile::builder(
        account_id,
        region.clone(),
        DisplayName::from_raw("Concurrency Test"),
        Username::try_new("concurrent_user".to_string()).unwrap(),
    )
    .build();

    // Premier save (v1)
    repo.save(&profile, None).await.unwrap();

    // On simule deux instances chargées en mémoire avec la v1
    let mut instance_a = repo
        .fetch(&account_id, &region)
        .await
        .unwrap()
        .unwrap();
    let mut instance_b = repo
        .fetch(&account_id, &region)
        .await
        .unwrap()
        .unwrap();

    // A gagne la course et passe en v2
    instance_a.update_display_name(&region, DisplayName::from_raw("Winner")).unwrap();
    repo.save(&instance_a, None).await.unwrap();

    // B essaie de save alors qu'il a toujours sa v1 en mémoire
    instance_b.update_display_name(&region, DisplayName::from_raw("Loser")).unwrap();
    let result = repo.save(&instance_b, None).await;

    assert!(matches!(
        result,
        Err(DomainError::ConcurrencyConflict { .. })
    ));
}

#[tokio::test]
async fn test_unique_username_constraint() {
    let (repo, _c) = get_repo().await;
    let region = RegionCode::try_new("eu".to_string()).unwrap();
    let username = Username::try_new("unique_bob".to_string()).unwrap();

    let bob1 = Profile::builder(
        AccountId::new(),
        region.clone(),
        DisplayName::from_raw("Bob 1"),
        username.clone(),
    )
    .build();
    repo.save(&bob1, None)
        .await
        .expect("First save should work");

    let bob2 = Profile::builder(
        AccountId::new(),
        region.clone(),
        DisplayName::from_raw("Bob 2"),
        username,
    )
    .build();
    let result = repo.save(&bob2, None).await;

    // Vérifie que c'est bien une erreur
    assert!(
        result.is_err(),
        "Should have failed due to duplicate username"
    );

    // Optionnel : vérifier que c'est la bonne erreur (selon ton mapping SqlxErrorExt)
    println!("{:?}", result.err());
}

#[tokio::test]
async fn test_partial_update_integrity() {
    let (repo, _c) = get_repo().await;
    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu".to_string()).unwrap();

    // 1. Création avec bio et avatar
    let mut profile = Profile::builder(
        account_id,
        region.clone(),
        DisplayName::from_raw("Alice"),
        Username::try_new("alice").unwrap(),
    )
    .build();
    profile.update_bio(&region, Some(Bio::try_new("Ma bio initiale").unwrap())).unwrap();
    profile.update_avatar(&region, Url::try_new("https://avatar.com/1").unwrap()).unwrap();
    repo.save(&profile, None).await.unwrap();

    // 2. Update uniquement du display_name
    let mut to_update = repo
        .fetch(&account_id, &region)
        .await
        .unwrap()
        .unwrap();
    to_update.update_display_name(&region, DisplayName::from_raw("Alice Nouvelle")).unwrap();
    repo.save(&to_update, None).await.unwrap();

    // 3. Vérification : La bio et l'avatar n'ont pas disparu (pas de NULL accidentel)
    let final_p = repo
        .fetch(&account_id, &region)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(final_p.display_name().as_str(), "Alice Nouvelle");
    assert_eq!(
        final_p.bio().as_ref().map(|b| b.as_str()),
        Some("Ma bio initiale")
    );
    assert_eq!(
        final_p.avatar_url().as_ref().map(|b| b.as_str()),
        Some("https://avatar.com/1")
    );
}

#[tokio::test]
async fn test_transaction_rollback_logic() {
    let (pool, _c) = crate::common::setup_postgres_test_db().await;
    let repo = PostgresProfileRepository::new(pool.clone());
    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu".to_string()).unwrap();

    // 1. On démarre la transaction SQLx brute
    let tx_sqlx = pool.begin().await.unwrap();

    // 2. On l'enveloppe dans ton wrapper infrastructure
    // C'est cette étape qui manquait !
    let mut wrapped_tx =
        shared_kernel::infrastructure::postgres::transactions::PostgresTransaction::new(tx_sqlx);

    let profile = Profile::builder(
        account_id,
        region.clone(),
        DisplayName::from_raw("Ghost"),
        Username::try_new("ghost").unwrap(),
    )
    .build();

    // 3. On passe le wrapper (qui implémente le trait Transaction)
    repo.save(&profile, Some(&mut wrapped_tx)).await.unwrap();

    let tx_to_rollback = wrapped_tx.into_inner();
    tx_to_rollback.rollback().await.unwrap();

    // 5. Vérification
    let found = repo.fetch(&account_id, &region).await.unwrap();
    assert!(
        found.is_none(),
        "Le profil ne devrait pas exister après un rollback"
    );
}
