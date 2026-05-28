#[cfg(test)]
mod integration_tests {
    use chrono::Utc;
    use infra_test::ScyllaTestContext;
    use shared_kernel::core::{Error, Identifier, Result};
    use shared_kernel::types::{Counter, ProfileId, Region, RegionCode};
    use social::entities::ProfileCounters;
    use social::repositories::CounterRepository;
    use social::scylla::ScyllaCounterRepository;

    /// Helper pour instancier le dépôt connecté au cluster éphémère de test
    async fn get_test_context() -> (ScyllaCounterRepository, ScyllaTestContext) {
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

        let repo = ScyllaCounterRepository::new(scylla_ctx.session().clone())
            .await
            .expect("Échec de l'initialisation du ScyllaCounterRepository");

        (repo, scylla_ctx)
    }

    #[tokio::test]
    async fn test_counter_fallback_when_no_row_exists() -> Result<()> {
        // --- Arrange ---
        let (repo, _scylla_ctx) = get_test_context().await;
        let random_profile_id = ProfileId::generate();

        // --- Act ---
        let counters = repo.get_counters(random_profile_id).await?;

        // --- Assert ---
        // Le pattern fallback doit renvoyer un agrégat propre initialisé à 0 sans crasher
        assert_eq!(counters.profile_id(), &random_profile_id);
        assert_eq!(counters.followers_count().value(), 0);
        assert_eq!(counters.following_count().value(), 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_counter_atomic_increment_and_decrement_lifecycle() -> Result<()> {
        // --- Arrange ---
        let (repo, _scylla_ctx) = get_test_context().await;

        let follower_id = ProfileId::generate();
        let following_id = ProfileId::generate();

        // --- Act: Étape 1 - Incrémentation atomique (Batch Logged) ---
        repo.increment_counters(follower_id, following_id).await?;

        // --- Assert: Étape 2 - Vérification de la double écriture ---
        let follower_counters = repo.get_counters(follower_id).await?;
        assert_eq!(
            follower_counters.following_count().value(),
            1,
            "Le follower doit avoir 1 following"
        );
        assert_eq!(
            follower_counters.followers_count().value(),
            0,
            "Le follower doit avoir 0 followers"
        );

        let following_counters = repo.get_counters(following_id).await?;
        assert_eq!(
            following_counters.followers_count().value(),
            1,
            "La cible doit avoir 1 follower"
        );
        assert_eq!(
            following_counters.following_count().value(),
            0,
            "La cible doit avoir 0 followings"
        );

        // --- Act: Étape 3 - Décrémentation atomique ---
        repo.decrement_counters(follower_id, following_id).await?;

        // --- Assert: Étape 4 - Retour à l'état initial ---
        let follower_counters_after = repo.get_counters(follower_id).await?;
        assert_eq!(follower_counters_after.following_count().value(), 0);

        let following_counters_after = repo.get_counters(following_id).await?;
        assert_eq!(following_counters_after.followers_count().value(), 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_counter_save_custom_delta_sync() -> Result<()> {
        // --- Arrange ---
        let (repo, _scylla_ctx) = get_test_context().await;
        let region = Region::from_raw(RegionCode::EU);
        let profile_id = ProfileId::generate();

        // Simulation d'un delta cumulé par un worker de réconciliation (+5 followers, +12 followings)
        let delta_counters = ProfileCounters::restore(
            profile_id,
            Counter::from_raw(5),
            Counter::from_raw(12),
            1,
            Utc::now(),
            Utc::now(),
        );

        // --- Act ---
        repo.save(&delta_counters).await?;

        // --- Assert ---
        let final_counters = repo.get_counters(profile_id).await?;
        assert_eq!(final_counters.followers_count().value(), 5);
        assert_eq!(final_counters.following_count().value(), 12);

        // Une deuxième sauvegarde de delta doit s'ajouter atomiquement (Cql Counter)
        repo.save(&delta_counters).await?;

        let aggregated_counters = repo.get_counters(profile_id).await?;
        assert_eq!(aggregated_counters.followers_count().value(), 10);
        assert_eq!(aggregated_counters.following_count().value(), 24);

        Ok(())
    }

    #[tokio::test]
    async fn test_counter_save_should_noop_when_deltas_are_zero() -> Result<()> {
        // --- Arrange ---
        let (repo, _scylla_ctx) = get_test_context().await;
        let region = Region::from_raw(RegionCode::EU);
        let profile_id = ProfileId::generate();

        let zero_counters = ProfileCounters::restore(
            profile_id,
            Counter::from_raw(0),
            Counter::from_raw(0),
            1,
            Utc::now(),
            Utc::now(),
        );

        // --- Act ---
        repo.save(&zero_counters).await?;

        // --- Assert ---
        // Aucune ligne ne doit être créée dans ScyllaDB si le delta est nul
        let res = _scylla_ctx
            .session()
            .query_unpaged(
                "SELECT profile_id FROM profile_counters WHERE profile_id = ?",
                (profile_id.as_uuid(),),
            )
            .await
            .map_err(|e| Error::internal(e.to_string()))?;

        assert_eq!(
            res.into_rows_result().unwrap().rows_num(),
            0,
            "La ligne ne devrait pas exister en base"
        );
        Ok(())
    }
}
