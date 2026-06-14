use chrono::{Duration, Utc};
use geo_discovery::{
    repositories::{MapAnnotationDiscoveryRepository, MapAnnotationArchiveRepository},
    types::PopularityScore,
};
use h3o::{LatLng, Resolution};
use std::str::FromStr;
use uuid::Uuid;

use geo_discovery::context::GeoDiscoveryCommandCtx;
use geo_discovery::types::{BucketHour, TileH3, TileResolution};
use geo_discovery::use_cases::IndexMapAnnotationCommand;
use geo_discovery_test_utils::GeoDiscoveryTestFixture;

use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result};
use shared_kernel::geo::GeoPoint;
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::types::{PostId, ProfileId, Region};

#[tokio::test]
async fn test_index_active_post_handler_success() -> Result<()> {
    // Arrange
    let f = GeoDiscoveryTestFixture::new().await;
    let command_id = Uuid::now_v7();
    let post_id = PostId::generate();
    let profile_id = ProfileId::generate();

    let lat = 48.8566;
    let lon = 2.3522;
    let location = GeoPoint::try_new(lat, lon).unwrap();

    let created_at = Utc::now();
    let expires_at = created_at + Duration::hours(4);

    let cmd = IndexMapAnnotationCommand {
        command_id,
        target: CommandTarget::stateless(profile_id),
        region: f.region(),
        post_id,
        location,
        post_type: "video".to_string(),
        thumbnail_url: Some("https://cdn.example.com/thumb.jpg".to_string()),
        created_at,
        expires_at,
        popularity_score: PopularityScore::from_raw(100.0),
    };

    // On s'assure que les stubs sont vides au départ
    assert_eq!(f.map_repo().count_all(), 0);

    // Act
    f.bus()
        .execute::<GeoDiscoveryCommandCtx, IndexMapAnnotationCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    // Assert - 1. Vérification ScyllaDB via le compteur interne
    assert_eq!(
        f.map_repo().count_all(),
        1,
        "Le handler aurait dû insérer exactement 1 enregistrement dans ScyllaDB"
    );

    // Assert - 2. Vérification de la multi-indexation Redis de manière découplée
    // On récupère toutes les tuiles que le handler a marquées comme actives dans le Set global Redis
    let active_tiles = f.map_cache_repo().get_all_active_tiles().await?;

    let expected_resolutions = vec![3, 5, 7, 9, 10];
    for res_val in expected_resolutions {
        // On vérifie qu'il y a au moins une tuile active enregistrée pour cette résolution
        let has_resolution = active_tiles
            .iter()
            .any(|(res, _tile)| res.value() == res_val);

        assert!(
            has_resolution,
            "Le cache Redis aurait dû tracker et indexer une tuile pour la résolution {}",
            res_val
        );
    }

    // Assert - 3. Vérification de la barrière d'idempotence
    assert!(
        f.idempotency_repo().exists(None, &command_id).await?,
        "La commande aurait dû être enregistrée dans le dépôt d'idempotence"
    );

    Ok(())
}

#[tokio::test]
async fn test_index_active_post_handler_idempotency_barrier() -> Result<()> {
    // Arrange
    let f = GeoDiscoveryTestFixture::new().await;
    let command_id = Uuid::now_v7();
    let post_id = PostId::generate();
    let profile_id = ProfileId::generate();

    let lat = 48.8156;
    let lon = 2.3204;
    let location = GeoPoint::try_new(lat, lon).unwrap();
    let created_at = Utc::now();

    // On pré-enregistre l'identifiant de la commande pour simuler un doublon réseau
    f.idempotency_repo().save(None, &command_id).await?;

    let cmd = IndexMapAnnotationCommand {
        command_id,
        target: CommandTarget::stateless(profile_id),
        region: f.region(),
        post_id,
        location,
        post_type: "image".to_string(),
        thumbnail_url: None,
        created_at,
        expires_at: created_at + Duration::hours(2),
        popularity_score: PopularityScore::from_raw(50.0),
    };

    // Act
    f.bus()
        .execute::<GeoDiscoveryCommandCtx, IndexMapAnnotationCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    // Assert - Comme l'idempotence a coupé court, aucune tuile ne doit contenir le post
    let h3_lat_lng = LatLng::new(lat, lon).unwrap();
    let cell_7 = h3_lat_lng.to_cell(Resolution::Seven);
    let tile_7 = TileH3::from_str(&cell_7.to_string()).unwrap();
    let bucket = BucketHour::from_timestamp(created_at.timestamp_millis());

    // Le stockage ScyllaDB doit être désert
    let records = f
        .map_repo()
        .find_by_tile(TileResolution::try_new(7).unwrap(), &tile_7, bucket)
        .await?;
    assert!(
        records.is_empty(),
        "L'idempotence a échoué: des données ont été persistées"
    );

    // Le cache Redis doit être désert
    let cached_posts = f
        .map_cache_repo()
        .get_top_posts(TileResolution::try_new(7).unwrap(), &tile_7, 50)
        .await?;
    assert_eq!(
        cached_posts.len(),
        0,
        "L'idempotence a échoué: des données ont fui dans Redis"
    );

    Ok(())
}

#[tokio::test]
async fn test_index_active_post_handler_region_sharding_violation() -> Result<()> {
    // Arrange
    let f = GeoDiscoveryTestFixture::new().await;
    let command_id = Uuid::now_v7();
    let profile_id = ProfileId::generate();

    // On forge délibérément une région différente de celle du contexte (Geo-Sharding violation)
    let invalid_region = Region::from_str("US").unwrap();
    assert_ne!(f.region(), invalid_region);

    let cmd = IndexMapAnnotationCommand {
        command_id,
        target: CommandTarget::stateless(profile_id),
        region: invalid_region,
        post_id: PostId::generate(),
        location: GeoPoint::try_new(48.8156, 2.3204).unwrap(),
        post_type: "text".to_string(),
        thumbnail_url: None,
        created_at: Utc::now(),
        expires_at: Utc::now() + Duration::hours(1),
        popularity_score: PopularityScore::from_raw(10.0),
    };

    // Act
    let result = f
        .bus()
        .execute::<GeoDiscoveryCommandCtx, IndexMapAnnotationCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await;

    // Assert
    assert!(
        result.is_err(),
        "Le handler aurait dû lever une erreur de Sharding"
    );
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::ValidationFailed);
    assert!(err.message.contains("Validation failed"));

    Ok(())
}
