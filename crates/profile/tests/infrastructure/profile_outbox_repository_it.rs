// crates/profile/tests/infrastructure/profile_outbox_repository_it.rs

use chrono::Utc;
use uuid::Uuid;
use profile::domain::builders::ProfileBuilder;
use profile::domain::entities::Profile;
use profile::domain::events::ProfileEvent;
use profile::domain::repositories::ProfileIdentityRepository;
use profile::domain::value_objects::{Bio, DisplayName};
use profile::infrastructure::postgres::repositories::PostgresProfileRepository;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::infrastructure::postgres::repositories::PostgresOutboxRepository;
use shared_kernel::infrastructure::postgres::transactions::PostgresTransaction;
use shared_kernel::domain::value_objects::{AccountId, Username, RegionCode};

#[tokio::test]
async fn test_outbox_with_real_profile_events() {
    let (pool, _c) = crate::common::setup_test_db().await;
    let repo = PostgresOutboxRepository::new(pool.clone());

    let account_id = AccountId::new();
    let region = RegionCode::from_raw("eu".to_string());

    let event = ProfileEvent::UsernameChanged {
        id: Uuid::new_v4(),
        account_id: account_id.clone(),
        region: region.clone(), // <--- AJOUTÉ ICI
        old_username: Username::try_new("old_bob").unwrap(),
        new_username: Username::try_new("new_bob").unwrap(),
        occurred_at: Utc::now(),
    };

    // 2. Transaction
    let tx_sqlx = pool.begin().await.unwrap();
    let mut wrapped_tx = PostgresTransaction::new(tx_sqlx);

    // 3. Save
    // On passe l'événement qui contient maintenant sa région
    repo.save(&mut wrapped_tx, &event).await.expect("Save failed");

    wrapped_tx.into_inner().commit().await.unwrap();

    // 4. Vérification
    // On vérifie aussi que la colonne region_code en DB est bien remplie
    let row: (serde_json::Value, String) = sqlx::query_as(
        "SELECT payload, region_code FROM outbox_events WHERE aggregate_id = $1"
    )
        .bind(account_id.to_string())
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(row.0["type"], "UsernameChanged");
    assert_eq!(row.0["data"]["new_username"], "new_bob");
    assert_eq!(row.1, "eu"); // On valide que la région est bien persistée hors du JSON
}

#[tokio::test]
async fn test_outbox_atomic_rollback_with_profile() {
    let (pool, _c) = crate::common::setup_test_db().await;
    let profile_repo = PostgresProfileRepository::new(pool.clone());
    let outbox_repo = PostgresOutboxRepository::new(pool.clone());

    // 1. Création (Génère ProfileCreated)
    let mut profile = Profile::create(
            ProfileBuilder::new(
            AccountId::new(),
            RegionCode::from_raw("eu"),
            DisplayName::from_raw("Ghost"),
            Username::try_new("ghost").unwrap()
        ).build()
    );

    // 2. On tire les events UNE SEULE FOIS
    let events = profile.pull_events();
    let event = events.first().expect("L'événement ProfileCreated devrait être présent");

    // 3. TRANSACTION
    let tx_sqlx = pool.begin().await.unwrap();
    let mut wrapped_tx = PostgresTransaction::new(tx_sqlx);

    // On sauve
    profile_repo.save(&profile, Some(&mut wrapped_tx)).await.unwrap();
    outbox_repo.save(&mut wrapped_tx, event.as_ref()).await.unwrap();

    // 4. ROLLBACK
    wrapped_tx.into_inner().rollback().await.unwrap();

    // 5. VERIFICATIONS
    let p_found = profile_repo.find_by_id(&profile.account_id(), &profile.region_code()).await.unwrap();
    assert!(p_found.is_none(), "Le profil ne devrait pas exister après rollback");

    let e_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM outbox_events WHERE aggregate_id = $1")
        .bind(profile.account_id().to_string())
        .fetch_one(&pool).await.unwrap();
    assert_eq!(e_count.0, 0, "L'événement ne devrait pas exister après rollback");
}

#[tokio::test]
async fn test_outbox_payload_integrity() {
    let (pool, _c) = crate::common::setup_test_db().await;
    let repo = PostgresOutboxRepository::new(pool.clone());

    // Test avec un changement de Bio (Option<Bio>)
    let event = ProfileEvent::BioUpdated {
        id: Uuid::new_v4(),
        account_id: AccountId::new(),
        region: RegionCode::from_raw("us"),
        old_bio: None,
        new_bio: Bio::try_new("Ma nouvelle bio 🚀").ok(),
        occurred_at: Utc::now(),
    };

    let mut tx = PostgresTransaction::new(pool.begin().await.unwrap());
    repo.save(&mut tx, &event).await.unwrap();
    tx.into_inner().commit().await.unwrap();

    let row: (serde_json::Value,) = sqlx::query_as("SELECT payload FROM outbox_events")
        .fetch_one(&pool).await.unwrap();

    // On vérifie que le tag "type" et le contenu "data" sont là (via ton attribut serde)
    assert_eq!(row.0["type"], "BioUpdated");
    assert!(row.0["data"]["new_bio"].is_string());
    assert_eq!(row.0["data"]["new_bio"], "Ma nouvelle bio 🚀");
}

#[tokio::test]
async fn test_outbox_duplicate_prevention() {
    let (pool, _c) = crate::common::setup_test_db().await;
    let repo = PostgresOutboxRepository::new(pool.clone());

    let event_id = Uuid::now_v7();
    let account_id = AccountId::new();
    let region = RegionCode::from_raw("eu".to_string());

    // On crée l'événement manuellement pour contrôler l'ID
    let event = ProfileEvent::UsernameChanged {
        id: event_id,
        account_id: account_id.clone(),
        region: region.clone(),
        old_username: Username::try_new("old_bob").unwrap(),
        new_username: Username::try_new("new_bob").unwrap(),
        occurred_at: Utc::now(),
    };

    // 1. Première insertion : succès attendu
    let tx1_sqlx = pool.begin().await.unwrap();
    let mut tx1 = PostgresTransaction::new(tx1_sqlx);
    repo.save(&mut tx1, &event).await.expect("La première sauvegarde devrait réussir");
    tx1.into_inner().commit().await.unwrap();

    // 2. Deuxième tentative avec le même événement (même ID + même Région)
    let tx2_sqlx = pool.begin().await.unwrap();
    let mut tx2 = PostgresTransaction::new(tx2_sqlx);
    let result = repo.save(&mut tx2, &event).await;

    // 3. Vérification : Postgres doit lever une erreur de violation de clé primaire
    assert!(
        result.is_err(),
        "La DB aurait dû rejeter le doublon car la PK est (id, region_code)"
    );

    // Optionnel : vérifier que c'est bien une erreur d'infrastructure
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("duplicate key value"), "L'erreur devrait être un doublon de clé");
}


// Herlpers
fn create_test_profile() -> Profile {
    ProfileBuilder::new(
        AccountId::new(),
        RegionCode::from_raw("eu".to_string()),
        DisplayName::from_raw("Alice"),
        Username::try_new("alice_dev").unwrap(),
    )
        .bio(Bio::try_new("Rustacean & Architect").unwrap())
        .is_private(false)
        .build()
}