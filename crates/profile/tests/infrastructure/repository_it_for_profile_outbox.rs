// crates/profile/tests/infrastructure/repository_it_for_profile_outbox.rs

use chrono::Utc;
use profile::domain::entities::Profile;
use profile::domain::events::ProfileEvent;
use profile::domain::repositories::ProfileIdentityRepository;
use profile::domain::value_objects::{Bio, DisplayName, Handle, ProfileId};
use profile::infrastructure::postgres::repositories::PostgresIdentityRepository;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::infrastructure::postgres::repositories::PostgresOutboxRepository;
use shared_kernel::infrastructure::postgres::transactions::PostgresTransaction;
use uuid::Uuid;
use shared_kernel::infrastructure::postgres::utils::PostgresTestContext;

/// Helper local pour obtenir un contexte Postgres propre
async fn get_pg_context() -> PostgresTestContext {
    PostgresTestContext::builder()
        .with_migrations(&["./migrations/postgres"])
        .build()
        .await
}

#[tokio::test]
async fn test_outbox_with_real_profile_events() {
    let ctx = get_pg_context().await;
    let pool = ctx.pool();
    let repo = PostgresOutboxRepository::new(pool.clone());
    let owner_id = AccountId::new();
    let profile_id = ProfileId::new();
    let region = RegionCode::try_new("eu").unwrap();

    let event = ProfileEvent::HandleChanged {
        id: Uuid::new_v4(),
        profile_id: profile_id.clone(),
        owner_id: owner_id.clone(),
        region: region.clone(),
        old_handle: Handle::try_new("old_bob").unwrap(),
        new_handle: Handle::try_new("new_bob").unwrap(),
        occurred_at: Utc::now(),
    };

    // Transaction
    let tx_sqlx = pool.begin().await.unwrap();
    let mut wrapped_tx = PostgresTransaction::new(tx_sqlx);

    repo.save(&mut wrapped_tx, &event).await.expect("Save failed");
    wrapped_tx.into_inner().commit().await.unwrap();

    // Vérification
    let row: (serde_json::Value, String) =
        sqlx::query_as("SELECT payload, region_code FROM outbox_events WHERE aggregate_id = $1")
            .bind(profile_id.to_string())
            .fetch_one(&pool) // On passe une référence à la pool
            .await
            .unwrap();

    assert_eq!(row.0["type"], "HandleChanged");
    assert_eq!(row.1, "eu");
}

#[tokio::test]
async fn test_outbox_atomic_rollback_with_profile() {
    let ctx = get_pg_context().await;
    let pool = ctx.pool();
    let profile_repo = PostgresIdentityRepository::new(pool.clone());
    let outbox_repo = PostgresOutboxRepository::new(pool.clone());

    let mut profile = Profile::create(create_test_profile());
    let profile_id = profile.id().clone();
    let event = profile.pull_events().first().cloned().expect("Event missing");

    // TRANSACTION
    let tx_sqlx = pool.begin().await.unwrap();
    let mut wrapped_tx = PostgresTransaction::new(tx_sqlx);

    profile_repo.save(&profile, Some(&mut wrapped_tx)).await.unwrap();
    outbox_repo.save(&mut wrapped_tx, event.as_ref()).await.unwrap();

    // ROLLBACK
    wrapped_tx.into_inner().rollback().await.unwrap();

    // VERIFICATIONS : Rien ne doit exister
    let p_found = profile_repo.fetch(&profile_id, &profile.region_code()).await.unwrap();
    assert!(p_found.is_none());

    let e_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM outbox_events WHERE aggregate_id = $1")
            .bind(profile_id.to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(e_count.0, 0);
}

#[tokio::test]
async fn test_outbox_payload_integrity() {
    let ctx = get_pg_context().await;
    let pool = ctx.pool();
    let repo = PostgresOutboxRepository::new(pool.clone());
    let owner_id = AccountId::new();

    let event = ProfileEvent::BioUpdated {
        id: Uuid::new_v4(),
        profile_id: ProfileId::new(),
        owner_id: owner_id.clone(),
        region: RegionCode::try_new("us").unwrap(),
        old_bio: None,
        new_bio: Bio::try_new("Ma nouvelle bio 🚀").ok(),
        occurred_at: Utc::now(),
    };

    // 1. Démarrage de la transaction via la pool du contexte
    let mut tx = PostgresTransaction::new(pool.begin().await.unwrap());

    // 2. Sauvegarde de l'événement
    repo.save(&mut tx, &event).await.unwrap();
    tx.into_inner().commit().await.unwrap();

    // 3. Vérification de la structure JSON persistée
    let row: (serde_json::Value,) = sqlx::query_as("SELECT payload FROM outbox_events")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(row.0["type"], "BioUpdated");
    assert_eq!(row.0["data"]["new_bio"], "Ma nouvelle bio 🚀");
}

#[tokio::test]
async fn test_outbox_duplicate_prevention() {
    let ctx = get_pg_context().await;
    let pool = ctx.pool();
    let repo = PostgresOutboxRepository::new(pool.clone());

    let event_id = Uuid::now_v7();
    let event = ProfileEvent::HandleChanged {
        id: event_id,
        profile_id: ProfileId::new(),
        owner_id: AccountId::new(),
        region: RegionCode::try_new("eu").unwrap(),
        old_handle: Handle::try_new("old").unwrap(),
        new_handle: Handle::try_new("new").unwrap(),
        occurred_at: Utc::now(),
    };

    let mut tx1 = PostgresTransaction::new(pool.begin().await.unwrap());
    repo.save(&mut tx1, &event).await.unwrap();
    tx1.into_inner().commit().await.unwrap();

    let mut tx2 = PostgresTransaction::new(pool.begin().await.unwrap());
    let result = repo.save(&mut tx2, &event).await;

    assert!(result.is_err(), "Duplicate event ID should be rejected by DB unique constraint");
}

// Helpers
fn create_test_profile() -> Profile {
    Profile::builder(
        AccountId::new(),
        RegionCode::try_new("eu").unwrap(),
        DisplayName::from_raw("Alice"),
        Handle::try_new("alice_dev").unwrap(),
    )
        .with_bio(Bio::try_new("Rustacean & Architect").unwrap())
        .build()
}