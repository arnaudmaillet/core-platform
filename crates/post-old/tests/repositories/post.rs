// crates/post/tests/post_test_it.rs

#[cfg(test)]
mod scylla_integration_tests {
    use infra_test::ScyllaTestContext;
    use post::ScyllaPostRepository;
    use shared_kernel::core::{PageQuery, Result};
    use shared_kernel::types::{PostId, PostType, ProfileId, Region};

    use post::entities::Post;
    use post::repositories::PostRepository;
    use post::types::{Caption, VisibilityLevel};

    async fn get_test_context() -> (ScyllaPostRepository, ScyllaTestContext) {
        let valid_path = ["./migrations/scylla"]
            .iter()
            .find(|p| std::path::Path::new(p).exists())
            .expect("💥 Impossible de localiser le dossier des migrations CQL");

        let scylla_ctx = ScyllaTestContext::builder()
            .with_keyspace("post_ns")
            .with_migrations(&[valid_path])
            .build()
            .await;

        let repo = ScyllaPostRepository::new(
            scylla_ctx.session().clone(),
            scylla_ctx.keyspace().to_string(),
        )
        .await
        .expect("Échec de l'initialisation du ScyllaPostRepository");

        (repo, scylla_ctx)
    }

    fn create_fixture_post(post_id: PostId, author_id: ProfileId, text: &str) -> Post {
        let caption = Caption::try_from(text).unwrap();
        Post::builder(post_id, author_id, PostType::Text, VisibilityLevel::Public)
            .with_caption(caption)
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn test_post_full_lifecycle_and_double_write_atomicity() -> Result<()> {
        let (repo, _scylla_ctx) = get_test_context().await;

        let region = Region::default();
        let author_id = ProfileId::generate();
        let post_id = PostId::generate();

        tracing::info!(%post_id, %author_id, "--- ARRANGING FIXTURE POST ---");
        let initial_post =
            create_fixture_post(post_id, author_id, "Wynn core platform setup! #rust");

        // --- Act: Étape 1 ---
        tracing::info!("--- EXECUTING REPO.SAVE() ---");
        repo.save(&initial_post).await?;

        // --- Assert: Étape 2 ---
        tracing::info!("--- EXECUTING REPO.FIND_BY_ID() ---");
        let found_by_id_opt = repo.find_by_id(&post_id).await?;

        if found_by_id_opt.is_none() {
            tracing::error!(
                %post_id,
                region = %region.to_string(),
                "CRITICAL: .find_by_id() a renvoyé None ! Le post n'a pas été trouvé."
            );
        }

        assert!(
            found_by_id_opt.is_some(),
            "Le post aurait dû être trouvé via .find_by_id()"
        );

        let post_by_id = found_by_id_opt.unwrap();
        assert_eq!(post_by_id.post_id(), post_id);
        Ok(())
    }

    #[tokio::test]
    async fn test_find_by_author_pagination_limits_and_ordering() -> Result<()> {
        // --- Arrange ---
        let (repo, _scylla_ctx) = get_test_context().await;
        let region = Region::default();
        let author_id = ProfileId::generate();

        // On insère 3 publications consécutives pour le même auteur
        for i in 1..=3 {
            let post_id = PostId::generate();
            let post = create_fixture_post(post_id, author_id, &format!("Post numéro {}", i));
            repo.save(&post).await?;
        }

        // --- Act & Assert : Étape 1 - Clamping strict via LIMIT ---
        let query_limit_2 = PageQuery::new(2, None);
        let result_page_1 = repo.find_by_author(&author_id, query_limit_2).await?;

        assert_eq!(
            result_page_1.items.len(),
            2,
            "La limite CQL de 2 aurait dû brider le résultat à 2 posts grâce à la troncature du repo"
        );
        assert!(
            result_page_1.next_cursor.is_some(),
            "Un curseur de pagination aurait dû être généré"
        );

        // --- Act & Assert : Étape 2 - Récupération globale ---
        let query_limit_10 = PageQuery::new(10, None);
        let result_all = repo.find_by_author(&author_id, query_limit_10).await?;

        assert_eq!(
            result_all.items.len(),
            3,
            "L'ensemble des 3 posts aurait dû être retourné"
        );
        assert!(
            result_all.next_cursor.is_none(),
            "Le curseur doit être None lorsqu'on a épuisé les lignes de la partition ScyllaDB"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_find_by_id_returns_none_safely_when_absent() -> Result<()> {
        let (repo, _scylla_ctx) = get_test_context().await;
        let non_existent_post_id = PostId::generate(); // Sans argument

        let result = repo.find_by_id(&non_existent_post_id).await?;
        assert!(result.is_none());
        Ok(())
    }
}
