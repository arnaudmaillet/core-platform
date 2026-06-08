use chrono::{Duration, Utc};
use h3o::{LatLng, Resolution};
use std::str::FromStr;
use uuid::Uuid;

use geo_discovery::context::GeoDiscoveryCommandContext;
use geo_discovery::handlers::IndexActivePostCommand;
use geo_discovery::types::{BucketHour, TileH3, TileResolution};
use geo_discovery_test_utils::GeoDiscoveryTestFixture;

use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result};
use shared_kernel::geo::GeoPoint;
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::types::{PostId, ProfileId, Region};

#[tokio::test]
async fn test_index_active_post_handler_success() -> Result<()> {
    // Arrange
    let f = GeoDiscoveryTestFixture::new();
    let command_id = Uuid::now_v7();
    let post_id = PostId::generate();
    let profile_id = ProfileId::generate();

    // Coordonnées de test (Paris / Arcueil)
    let lat = 48.8566;
    let lon = 2.3522;
    let location = GeoPoint::try_new(lat, lon).unwrap();

    let created_at = Utc::now();
    let expires_at = created_at + Duration::hours(4);

    let cmd = IndexActivePostCommand {
        command_id,
        target: CommandTarget::stateless(profile_id),
        region: f.region(),
        post_id,
        location,
        post_type: "video".to_string(),
        thumbnail_url: Some("https://cdn.example.com/thumb.jpg".to_string()),
        created_at,
        expires_at,
        initial_score: 100.0,
    };

    // Act
    f.bus()
        .execute::<GeoDiscoveryCommandContext, IndexActivePostCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    // Assert - 1. Vérification de la persistance pivot ScyllaDB (Résolution 7)
    let h3_lat_lng = LatLng::new(lon, lat).unwrap();

    let scylla_cell = h3_lat_lng.to_cell(Resolution::Seven);
    let scylla_tile = TileH3::from_str(&scylla_cell.to_string()).unwrap();
    let bucket = BucketHour::from_timestamp(created_at.timestamp_millis());

    f.assert_persisted_post_exists(
        TileResolution::try_new(7).unwrap(),
        &scylla_tile,
        bucket,
        &post_id,
    )
    .await;

    // Assert - 2. Vérification de la multi-indexation Redis (Niveaux 3, 5, 7, 9, 10)
    let expected_resolutions = vec![3, 5, 7, 9, 10];
    for res_val in expected_resolutions {
        let res = TileResolution::try_new(res_val).unwrap();
        let h3_res = Resolution::try_from(res_val as u8).unwrap();
        let cell = h3_lat_lng.to_cell(h3_res);
        let tile_id = TileH3::from_str(&cell.to_string()).unwrap();

        // S'assurer que le post est bien présent dans la tuile du cache Redis
        f.assert_cache_post_count(res, &tile_id, 1).await;
    }

    // Assert - 3. Inscription dans la table d'idempotence
    assert!(f.idempotency_repo().exists(None, &command_id).await?);

    Ok(())
}

#[tokio::test]
async fn test_index_active_post_handler_idempotency_barrier() -> Result<()> {
    // Arrange
    let f = GeoDiscoveryTestFixture::new();
    let command_id = Uuid::now_v7();
    let post_id = PostId::generate();
    let profile_id = ProfileId::generate();

    let location = GeoPoint::try_new(48.8156, 2.3204).unwrap();
    let created_at = Utc::now();

    // On pré-enregistre l'identifiant de la commande pour simuler un doublon réseau
    f.idempotency_repo().save(None, &command_id).await?;

    let cmd = IndexActivePostCommand {
        command_id,
        target: CommandTarget::stateless(profile_id),
        region: f.region(),
        post_id,
        location,
        post_type: "image".to_string(),
        thumbnail_url: None,
        created_at,
        expires_at: created_at + Duration::hours(2),
        initial_score: 50.0,
    };

    // Act
    f.bus()
        .execute::<GeoDiscoveryCommandContext, IndexActivePostCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    // Assert - Comme l'idempotence a coupé court, aucune tuile ne doit contenir le post
    let h3_lat_lng = LatLng::new(48.8156, 2.3204).unwrap();
    let cell_7 = h3_lat_lng.to_cell(Resolution::Seven);
    let tile_7 = TileH3::from_str(&cell_7.to_string()).unwrap();
    let bucket = BucketHour::from_timestamp(created_at.timestamp_millis());

    // Le stockage ScyllaDB doit être désert
    use geo_discovery::repositories::MapPersistenceRepository;
    let records = f
        .persistence_repo()
        .find_by_tile(TileResolution::try_new(7).unwrap(), &tile_7, bucket)
        .await?;
    assert!(
        records.is_empty(),
        "L'idempotence a échoué: des données ont été persistées"
    );

    // Le cache Redis doit être désert
    f.assert_cache_post_count(TileResolution::try_new(7).unwrap(), &tile_7, 0)
        .await;

    Ok(())
}

#[tokio::test]
async fn test_index_active_post_handler_region_sharding_violation() -> Result<()> {
    // Arrange
    let f = GeoDiscoveryTestFixture::new();
    let command_id = Uuid::now_v7();
    let profile_id = ProfileId::generate();

    // On forge délibérément une région différente de celle du contexte (Geo-Sharding violation)
    let invalid_region = Region::from_str("US").unwrap();
    assert_ne!(f.region(), invalid_region);

    let cmd = IndexActivePostCommand {
        command_id,
        target: CommandTarget::stateless(profile_id),
        region: invalid_region,
        post_id: PostId::generate(),
        location: GeoPoint::try_new(48.8156, 2.3204).unwrap(),
        post_type: "text".to_string(),
        thumbnail_url: None,
        created_at: Utc::now(),
        expires_at: Utc::now() + Duration::hours(1),
        initial_score: 10.0,
    };

    // Act
    let result = f
        .bus()
        .execute::<GeoDiscoveryCommandContext, IndexActivePostCommand, ()>(
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
