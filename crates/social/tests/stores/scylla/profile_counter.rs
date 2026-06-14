// crates/social/tests/scylla_counters_test.rs (à adapter selon ton arborescence)

use chrono::Utc;
use infra_test::ScyllaTestContext;
use shared_kernel::core::{Error, Identifier, Result};
use shared_kernel::types::{Counter, ProfileId};
use social::entities::ProfileCounters;
use social::repositories::ProfileCountersStorageRepository;
use social::stores::ScyllaProfileCountersStore;

/// Helper pour instancier le dépôt connecté au cluster éphémère de test
async fn get_test_context() -> (ScyllaProfileCountersStore, ScyllaTestContext) {
    let possible_migration_dirs = ["./migrations/scylla"];
    let valid_dir = possible_migration_dirs
        .iter()
        .find(|p| std::path::Path::new(p).is_dir())
        .expect("💥 Impossible de localiser le dossier des migrations CQL");

    let scylla_ctx = ScyllaTestContext::builder()
        .with_keyspace("counter_ns")
        .with_migrations(&[valid_dir])
        .build()
        .await;

    let repo = ScyllaProfileCountersStore::new(scylla_ctx.session().clone())
        .await
        .expect("Échec de l'initialisation du ScyllaCounterRepository");

    (repo, scylla_ctx)
}

#[tokio::test]
async fn test_counter_fetch_should_return_none_when_no_row_exists() -> Result<()> {
    // --- Arrange ---
    let (repo, _scylla_ctx) = get_test_context().await;
    let random_profile_id = ProfileId::generate();

    // --- Act ---
    let counters_opt = repo.fetch(random_profile_id).await?;

    // --- Assert ---
    assert!(counters_opt.is_none());
    Ok(())
}

#[tokio::test]
async fn test_counter_save_custom_delta_sync() -> Result<()> {
    // --- Arrange ---
    let (repo, _scylla_ctx) = get_test_context().await;
    let profile_id = ProfileId::generate();

    // Simulation d'un delta cumulé par un worker (+5 followers, +12 followings)
    let delta_counters = ProfileCounters::restore(
        profile_id,
        Counter::from_raw(5),
        Counter::from_raw(12),
        Utc::now(),
    );

    // --- Act ---
    repo.commit_deltas(&delta_counters).await?;

    // --- Assert ---
    let final_counters = repo.fetch(profile_id).await?.unwrap();
    assert_eq!(final_counters.followers_count().value(), 5);
    assert_eq!(final_counters.following_count().value(), 12);

    // Une deuxième sauvegarde de delta doit s'ajouter atomiquement (Cql Counter native)
    repo.commit_deltas(&delta_counters).await?;

    let aggregated_counters = repo.fetch(profile_id).await?.unwrap();
    assert_eq!(aggregated_counters.followers_count().value(), 10);
    assert_eq!(aggregated_counters.following_count().value(), 24);

    Ok(())
}

#[tokio::test]
async fn test_counter_save_should_noop_when_deltas_are_zero() -> Result<()> {
    // --- Arrange ---
    let (repo, _scylla_ctx) = get_test_context().await;
    let profile_id = ProfileId::generate();

    let zero_counters = ProfileCounters::restore(
        profile_id,
        Counter::from_raw(0),
        Counter::from_raw(0),
        Utc::now(),
    );

    // --- Act ---
    repo.commit_deltas(&zero_counters).await?;

    // --- Assert ---
    let res = _scylla_ctx
        .session()
        .query_unpaged(
            "SELECT profile_id FROM profile_counters WHERE profile_id = ?",
            (profile_id.as_uuid(),),
        )
        .await
        .map_err(|e| Error::internal(e.to_string()))?;

    assert_eq!(res.into_rows_result().unwrap().rows_num(), 0);
    Ok(())
}
