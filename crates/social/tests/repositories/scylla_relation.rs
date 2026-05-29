#[cfg(test)]
mod integration_tests {
    use infra_test::ScyllaTestContext;
    use shared_kernel::core::{Result, Versioned};
    use shared_kernel::types::ProfileId;
    use social::entities::FollowRelation;
    use social::repositories::RelationRepository;
    use social::scylla::ScyllaRelationRepository;

    /// Helper pour instancier le dépôt ScyllaDB connecté à un cluster de test ou local éphémère
    async fn get_test_context() -> (ScyllaRelationRepository, ScyllaTestContext) {
        let possible_migration_paths = ["./migrations/scylla"];

        let valid_path = possible_migration_paths
            .iter()
            .find(|p| std::path::Path::new(p).exists())
            .expect(
                "💥 Impossible de localiser le dossier des migrations CQL pour le module social",
            );

        let scylla_ctx = ScyllaTestContext::builder()
            .with_keyspace("social_ns") // Nom de base court (max 16 chars)
            .with_migrations(&[valid_path])
            .build()
            .await;

        let repo = ScyllaRelationRepository::new(scylla_ctx.session().clone())
            .await
            .expect("Échec de l'initialisation du ScyllaRelationRepository");

        (repo, scylla_ctx)
    }

    #[tokio::test]
    async fn test_follow_relation_full_lifecycle_and_batch_atomicity() -> Result<()> {
        // --- Arrange ---
        let (repo, _scylla_ctx) = get_test_context().await;

        let follower_id = ProfileId::generate();
        let following_id = ProfileId::generate();

        let relation = FollowRelation::builder(follower_id, following_id)
            .with_version(1)
            .build()?;

        // --- Act: Étape 1 (Sauvegarde via le Batch Logged ScyllaDB) ---
        repo.save(&relation).await?;

        // --- Assert: Étape 2 (Vérification de la double écriture synchrone) ---
        // 1. Validation de l'existence générale
        let exists = repo.is_following(follower_id, following_id).await?;
        assert!(exists, "is_following aurait dû renvoyer true");

        // 2. Validation de la reconstruction complète (Pattern Restore) depuis la table principale
        let found_opt = repo.find(follower_id, following_id).await?;
        assert!(
            found_opt.is_some(),
            "La relation aurait dû être trouvée via .find()"
        );

        let found = found_opt.unwrap();
        assert_eq!(found.version(), 1);
        assert_eq!(found.follower_id(), &follower_id);
        assert_eq!(found.following_id(), &following_id);

        // --- Assert: Étape 3 (Vérification des Index / Tables Miroirs) ---
        // On vérifie que la table miroir `followers` a bien été alimentée par le Batch
        let followers_list = repo.get_followers_ids(following_id, 10, 0).await?;
        assert_eq!(followers_list.len(), 1);
        assert_eq!(followers_list[0], follower_id);

        let following_list = repo.get_following_ids(follower_id, 10, 0).await?;
        assert_eq!(following_list.len(), 1);
        assert_eq!(following_list[0], following_id);

        // --- Act: Étape 4 (Suppression Atomique) ---
        repo.delete(follower_id, following_id).await?;

        // --- Assert: Étape 5 (Vérification de la purge totale) ---
        let missing_find = repo.find(follower_id, following_id).await?;
        assert!(
            missing_find.is_none(),
            "La relation aurait dû disparaître de la table principale"
        );

        let missing_is_following = repo.is_following(follower_id, following_id).await?;
        assert!(
            !missing_is_following,
            "is_following aurait dû repasser à false"
        );

        // Vérification du nettoyage de la table miroir
        let cleared_followers = repo.get_followers_ids(following_id, 10, 0).await?;
        assert!(
            cleared_followers.is_empty(),
            "La table miroir followers aurait dû être purgée"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_get_following_and_followers_pagination_limits() -> Result<()> {
        // --- Arrange ---
        let (repo, _scylla_ctx) = get_test_context().await;

        let target_user = ProfileId::generate();

        // On génère 5 followers différents qui vont follow notre target
        for _ in 0..5 {
            let follower_id = ProfileId::generate();
            let relation = FollowRelation::builder(follower_id, target_user).build()?;
            repo.save(&relation).await?;
        }

        // --- Act & Assert ---
        // On teste le paramètre de limite (Clamping des lignes CQL)
        let limited_followers = repo.get_followers_ids(target_user, 3, 0).await?;
        assert_eq!(
            limited_followers.len(),
            3,
            "La requête ScyllaDB aurait dû être limitée à 3 lignes (LIMIT 3)"
        );

        let all_followers = repo.get_followers_ids(target_user, 10, 0).await?;
        assert_eq!(all_followers.len(), 5);

        Ok(())
    }

    #[tokio::test]
    async fn test_find_should_return_none_safely_when_no_data_exists() -> Result<()> {
        // --- Arrange ---
        let (repo, _scylla_ctx) = get_test_context().await;

        let random_follower = ProfileId::generate();
        let random_following = ProfileId::generate();

        // --- Act ---
        let result = repo.find(random_follower, random_following).await?;

        // --- Assert ---
        assert!(
            result.is_none(),
            "Le find NoSQL ne doit pas lever d'erreur sur clé absente, mais renvoyer None"
        );
        Ok(())
    }
}
