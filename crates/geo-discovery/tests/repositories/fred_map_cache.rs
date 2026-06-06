// crates/geo_discovery/tests/fred_map_cache_it.rs

#[cfg(test)]
mod redis_cache_integration_tests {
    use chrono::{Duration, Utc};
    use infra_test::RedisTestContext;
    use std::str::FromStr;

    use geo_discovery::db::FredMapCacheRepository;
    use geo_discovery::repositories::MapCacheRepository;
    use geo_discovery::types::{TileH3, TilePostMetadata, TileResolution};

    use shared_kernel::core::Result;
    use shared_kernel::types::{PostId, PostType};

    /// Helper pour initialiser le repository connecté au conteneur Redis éphémère
    async fn get_test_context() -> (FredMapCacheRepository, RedisTestContext) {
        let redis_ctx = RedisTestContext::builder().build().await;
        let pool = redis_ctx.repository().pool().clone();

        let repo = FredMapCacheRepository::new(pool);
        (repo, redis_ctx)
    }

    #[tokio::test]
    async fn test_add_to_tile_and_get_top_posts_lifecycle() -> Result<()> {
        // Arrange
        let (repo, _redis_ctx) = get_test_context().await;

        let resolution = TileResolution::try_new(7).unwrap();
        let tile_id = TileH3::from_str("871fb4672ffffff").unwrap();

        let post_id_1 = PostId::generate();
        let post_id_2 = PostId::generate();

        let meta_1 = TilePostMetadata::new(
            post_id_1,
            48.8566,
            2.3522,
            PostType::Video,
            Some("https://cdn.com/v1.jpg".to_string()),
        );
        let meta_2 = TilePostMetadata::new(post_id_2, 48.8156, 2.3204, PostType::Image, None);

        let expires_at = Utc::now() + Duration::hours(2);

        // --- Act 1: Ajout au cache avec des scores de popularité différents ---
        repo.add_to_tile(resolution, &tile_id, &meta_1, 50.0, expires_at)
            .await?;
        repo.add_to_tile(resolution, &tile_id, &meta_2, 150.0, expires_at)
            .await?;

        // --- Act 2: Récupération du Top (Ordonné par score décroissant grâce à ZREVRANGE) ---
        let top_posts = repo.get_top_posts(resolution, &tile_id, 10).await?;

        // --- Assert 1: Classement et hydratation Protobuf ---
        assert_eq!(
            top_posts.len(),
            2,
            "La tuile Redis doit contenir exactement 2 publications"
        );

        // Le Post 2 doit être premier car son score (150.0) > Post 1 (50.0)
        let first_scored = &top_posts[0];
        assert_eq!(first_scored.metadata.post_id, post_id_2);
        assert_eq!(first_scored.metadata.post_type, PostType::Image);
        assert_eq!(first_scored.popularity_score.value(), 150.0);
        assert!(first_scored.metadata.thumbnail_url.is_none());

        let second_scored = &top_posts[1];
        assert_eq!(second_scored.metadata.post_id, post_id_1);
        assert_eq!(second_scored.metadata.post_type, PostType::Video);
        assert_eq!(
            second_scored.metadata.thumbnail_url,
            Some("https://cdn.com/v1.jpg".to_string())
        );

        // --- Act 3: Incrémentation dynamique du score ---
        repo.increment_score(resolution, &tile_id, &post_id_1, 200.0)
            .await?;

        // --- Assert 2: Inversion des positions suite à l'engagement (50.0 + 200.0 = 250.0)
        let updated_top = repo.get_top_posts(resolution, &tile_id, 1).await?;
        assert_eq!(
            updated_top[0].metadata.post_id, post_id_1,
            "Le Post 1 devrait être repassé en tête"
        );
        assert_eq!(updated_top[0].popularity_score.value(), 250.0);

        Ok(())
    }

    #[tokio::test]
    async fn test_remove_from_tile_cleans_sorted_sets_correctly() -> Result<()> {
        // Arrange
        let (repo, _redis_ctx) = get_test_context().await;
        let resolution = TileResolution::try_new(9).unwrap();
        let tile_id = TileH3::from_str("891fb4672ffffff").unwrap();
        let post_id = PostId::generate();
        let meta = TilePostMetadata::new(post_id, 48.8566, 2.3522, PostType::Text, None);

        repo.add_to_tile(
            resolution,
            &tile_id,
            &meta,
            10.0,
            Utc::now() + Duration::hours(1),
        )
        .await?;
        assert_eq!(repo.get_tile_post_count(resolution, &tile_id).await?, 1);

        // --- Act ---
        repo.remove_from_tile(resolution, &tile_id, &post_id)
            .await?;

        // --- Assert ---
        assert_eq!(
            repo.get_tile_post_count(resolution, &tile_id).await?,
            0,
            "Le ZSET de popularité doit être purgé"
        );

        let top = repo.get_top_posts(resolution, &tile_id, 10).await?;
        assert!(top.is_empty(), "La lecture doit renvoyer un vecteur vide");

        Ok(())
    }

    #[tokio::test]
    async fn test_evict_old_posts_filters_and_purges_only_expired() -> Result<()> {
        // Arrange
        let (repo, _redis_ctx) = get_test_context().await;
        let resolution = TileResolution::try_new(7).unwrap();
        let tile_id = TileH3::from_str("871fb4672ffffff").unwrap();

        let id_valid = PostId::generate();
        let id_expired = PostId::generate();

        let meta_valid = TilePostMetadata::new(id_valid, 48.8, 2.3, PostType::Video, None);
        let meta_expired = TilePostMetadata::new(id_expired, 48.8, 2.3, PostType::Video, None);

        let base_time = Utc::now();

        // Post valide expire dans 1 heure, post obsolète a expiré il y a 10 minutes
        repo.add_to_tile(
            resolution,
            &tile_id,
            &meta_valid,
            10.0,
            base_time + Duration::hours(1),
        )
        .await?;
        repo.add_to_tile(
            resolution,
            &tile_id,
            &meta_expired,
            10.0,
            base_time - Duration::minutes(10),
        )
        .await?;

        // --- Act : On demande l'éviction par rapport à l'instant présent ---
        let evicted = repo
            .evict_old_posts(resolution, &tile_id, base_time)
            .await?;

        // --- Assert ---
        assert_eq!(evicted.len(), 1, "Un seul post aurait dû être éjecté");
        assert_eq!(
            evicted[0].post_id, id_expired,
            "C'est le post obsolète qui doit être retourné"
        );

        // On vérifie qu'il ne reste bien que le post valide dans le ZSET
        assert_eq!(repo.get_tile_post_count(resolution, &tile_id).await?, 1);
        let current_top = repo.get_top_posts(resolution, &tile_id, 10).await?;
        assert_eq!(current_top[0].metadata.post_id, id_valid);

        Ok(())
    }

    #[tokio::test]
    async fn test_track_and_untrack_active_tiles_global_set() -> Result<()> {
        // Arrange
        let (repo, _redis_ctx) = get_test_context().await;
        let res_1 = TileResolution::try_new(5).unwrap();
        let res_2 = TileResolution::try_new(7).unwrap();

        let tile_1 = TileH3::from_str("851fb467fffffff").unwrap();
        let tile_2 = TileH3::from_str("871fb4672ffffff").unwrap();

        // --- Act 1: Enregistrement global de deux tuiles actives ---
        repo.track_active_tile(res_1, &tile_1).await?;
        repo.track_active_tile(res_2, &tile_2).await?;

        // --- Assert 1 ---
        let active_tiles = repo.get_all_active_tiles().await?;
        assert_eq!(
            active_tiles.len(),
            2,
            "Deux tuiles doivent être listées comme actives dans le Set Redis"
        );

        let contains_tile_1 = active_tiles
            .iter()
            .any(|(res, t)| res.value() == res_1.value() && t.value() == tile_1.value());
        let contains_tile_2 = active_tiles
            .iter()
            .any(|(res, t)| res.value() == res_2.value() && t.value() == tile_2.value());
        assert!(contains_tile_1);
        assert!(contains_tile_2);

        // --- Act 2: Désactivation d'une tuile ---
        repo.untrack_active_tile(res_1, &tile_1).await?;

        // --- Assert 2 ---
        let active_tiles_after = repo.get_all_active_tiles().await?;
        assert_eq!(active_tiles_after.len(), 1);
        assert_eq!(
            active_tiles_after[0].1.value(),
            tile_2.value(),
            "Seule la tuile 2 doit subsister"
        );

        Ok(())
    }
}
