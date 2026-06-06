use chrono::{Duration, Utc};
use std::str::FromStr;

use geo_discovery::entities::ActiveMapPost;
use geo_discovery::handlers::HydrateTileCacheCommand;
use geo_discovery::handlers::HydrateTileCacheHandler;
use geo_discovery::repositories::MapPersistenceRepository;
use geo_discovery::types::{TileH3, TileResolution};
use geo_discovery_test_utils::GeoDiscoveryTestFixture;

use shared_kernel::core::Result;
use shared_kernel::geo::GeoPoint;
use shared_kernel::types::{PostId, PostType};

#[tokio::test]
async fn test_hydrate_tile_cache_handler_success() -> Result<()> {
    // Arrange
    let f = GeoDiscoveryTestFixture::new();

    let resolution = TileResolution::try_new(7).unwrap();
    let tile_id = TileH3::from_str("871f1d48bffffff").unwrap();

    let now = Utc::now();

    let post_id_1 = PostId::generate();
    let post_id_2 = PostId::generate();
    let location = GeoPoint::try_new(48.8156, 2.3204).unwrap();

    // 1. GIVEN : On pré-remplit la persistance ScyllaDB avec 2 posts actifs valides
    let post_1 = ActiveMapPost::builder(post_id_1, location, resolution, tile_id.clone())
        .with_post_type(PostType::Video)
        .with_created_at(now - Duration::hours(1))
        .with_expires_at(now + Duration::hours(3))
        .build()
        .unwrap();

    let post_2 = ActiveMapPost::builder(post_id_2, location, resolution, tile_id.clone())
        .with_post_type(PostType::Image)
        .with_created_at(now - Duration::hours(2))
        .with_expires_at(now + Duration::hours(1))
        .build()
        .unwrap();

    // Sauvegarde manuelle dans le stub ScyllaDB
    f.persistence_repo()
        .save(&post_1, std::time::Duration::from_secs(3600))
        .await?;
    f.persistence_repo()
        .save(&post_2, std::time::Duration::from_secs(3600))
        .await?;

    // On s'assure que le cache Redis est totalement vide au départ
    f.assert_cache_post_count(resolution, &tile_id, 0).await;

    // 2. WHEN : On instancie et appelle directement le handler de réhydratation
    let handler = HydrateTileCacheHandler::new(f.cache_repo_dyn(), f.persistence_repo_dyn(), 50);
    let command = HydrateTileCacheCommand::new(resolution, tile_id.clone());
    handler.handle(command).await?;

    // 3. ASSERT : Le cache Redis doit maintenant contenir nos 2 posts valides
    f.assert_cache_post_count(resolution, &tile_id, 2).await;

    // On vérifie également que la tuile a bien été enregistrée globalement comme active
    use geo_discovery::repositories::MapCacheRepository;
    let active_tiles = f.cache_repo().get_all_active_tiles().await?;
    let tile_tracked = active_tiles
        .iter()
        .any(|(res, t)| res.value() == resolution.value() && t.value() == tile_id.value());
    assert!(
        tile_tracked,
        "La tuile aurait dû être marquée active dans le Set Redis globale"
    );

    Ok(())
}

#[tokio::test]
async fn test_hydrate_tile_cache_handler_skips_expired_posts() -> Result<()> {
    // Arrange
    let f = GeoDiscoveryTestFixture::new();

    let resolution = TileResolution::try_new(7).unwrap();
    let tile_id = TileH3::from_str("871f1d48bffffff").unwrap();

    let now = Utc::now();
    let post_id_expired = PostId::generate();
    let location = GeoPoint::try_new(48.8156, 2.3204).unwrap();

    // GIVEN : Un post persistant mais logiquement expiré (expires_at dans le passé)
    let expired_post =
        ActiveMapPost::builder(post_id_expired, location, resolution, tile_id.clone())
            .with_post_type(PostType::Text)
            .with_created_at(now - Duration::hours(5))
            .with_expires_at(now - Duration::hours(1)) // Expire il y a 1h
            .build()
            .unwrap();

    f.persistence_repo()
        .save(&expired_post, std::time::Duration::from_secs(0))
        .await?;

    // WHEN : On exécute l'hydratation
    let handler = HydrateTileCacheHandler::new(f.cache_repo_dyn(), f.persistence_repo_dyn(), 50);

    let command = HydrateTileCacheCommand::new(resolution, tile_id.clone());
    handler.handle(command).await?;

    // ASSERT : Le post expiré doit être filtré, le compteur Redis reste à 0
    f.assert_cache_post_count(resolution, &tile_id, 0).await;

    Ok(())
}
