use chrono::{Duration, Utc};
use h3o::{LatLng, Resolution};
use std::str::FromStr;
use uuid::Uuid;

use geo_discovery::context::GeoDiscoveryCommandContext;
use geo_discovery::handlers::{IndexActivePostCommand, RemovePostFromMapCommand};
use geo_discovery::types::{BucketHour, TileH3, TileResolution};
use geo_discovery_test_utils::GeoDiscoveryTestFixture;

use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result};
use shared_kernel::geo::GeoPoint;
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::types::{PostId, ProfileId, Region};

#[tokio::test]
async fn test_remove_post_from_map_handler_success() -> Result<()> {
    // Arrange
    let f = GeoDiscoveryTestFixture::new();
    let post_id = PostId::generate();
    let profile_id = ProfileId::generate();

    let lat = 48.8566;
    let lon = 2.3522;
    let location = GeoPoint::try_new(lat, lon).unwrap();
    let created_at = Utc::now();

    // 1. GIVEN : On indexe d'abord le post pour qu'il existe dans nos stubs d'infra
    let index_cmd = IndexActivePostCommand {
        command_id: Uuid::now_v7(),
        target: CommandTarget::stateless(profile_id, f.region()),
        post_id,
        location,
        post_type: "video".to_string(),
        thumbnail_url: None,
        created_at,
        expires_at: created_at + Duration::hours(2),
        initial_score: 10.0,
    };
    f.bus()
        .execute::<GeoDiscoveryCommandContext, IndexActivePostCommand, ()>(
            f.command_ctx().clone(),
            index_cmd,
        )
        .await?;

    // On valide qu'il est bien présent avant d'exécuter la suppression
    let h3_lat_lng = LatLng::new(lon, lat).unwrap();
    let cell_7 = h3_lat_lng.to_cell(Resolution::Seven);
    let tile_7 = TileH3::from_str(&cell_7.to_string()).unwrap();
    let bucket = BucketHour::from_timestamp(created_at.timestamp_millis());

    f.assert_persisted_post_exists(
        TileResolution::try_new(7).unwrap(),
        &tile_7,
        bucket,
        &post_id,
    )
    .await;

    // 2. WHEN : On exécute le handler de suppression
    let remove_command_id = Uuid::now_v7();
    let remove_cmd = RemovePostFromMapCommand {
        command_id: remove_command_id,
        target: CommandTarget::stateless(profile_id, f.region()),
        post_id,
        location,
        created_at,
    };

    f.bus()
        .execute::<GeoDiscoveryCommandContext, RemovePostFromMapCommand, ()>(
            f.command_ctx().clone(),
            remove_cmd,
        )
        .await?;

    // 3. ASSERT : Vérification de la suppression complète dans ScyllaDB
    use geo_discovery::repositories::MapPersistenceRepository;
    let records = f
        .persistence_repo()
        .find_by_tile(TileResolution::try_new(7).unwrap(), &tile_7, bucket)
        .await?;
    let found_in_scylla = records.iter().any(|p| p.post_id() == post_id);
    assert!(
        !found_in_scylla,
        "Le post n'a pas été supprimé de la persistance ScyllaDB"
    );

    // Vérification du nettoyage complet de tous les index de résolutions Redis
    let expected_resolutions = vec![3, 5, 7, 9, 10];
    for res_val in expected_resolutions {
        let res = TileResolution::try_new(res_val).unwrap();
        let h3_res = Resolution::try_from(res_val as u8).unwrap();
        let cell = h3_lat_lng.to_cell(h3_res);
        let tile_id = TileH3::from_str(&cell.to_string()).unwrap();

        // Toutes les tuiles Redis doivent être repassées à un décompte de 0 post actif
        f.assert_cache_post_count(res, &tile_id, 0).await;
    }

    // Inscription finale de la commande dans la table d'idempotence
    assert!(
        f.idempotency_repo()
            .exists(None, &remove_command_id)
            .await?
    );

    Ok(())
}

#[tokio::test]
async fn test_remove_post_from_map_handler_idempotency_barrier() -> Result<()> {
    // Arrange
    let f = GeoDiscoveryTestFixture::new();
    let command_id = Uuid::now_v7();
    let post_id = PostId::generate();
    let profile_id = ProfileId::generate();
    let location = GeoPoint::try_new(48.8156, 2.3204).unwrap();
    let created_at = Utc::now();

    // On injecte déjà la commande de suppression dans l'idempotence (doublon réseau)
    f.idempotency_repo().save(None, &command_id).await?;

    let remove_cmd = RemovePostFromMapCommand {
        command_id,
        target: CommandTarget::stateless(profile_id, f.region()),
        post_id,
        location,
        created_at,
    };

    // Act
    f.bus()
        .execute::<GeoDiscoveryCommandContext, RemovePostFromMapCommand, ()>(
            f.command_ctx().clone(),
            remove_cmd,
        )
        .await?;

    // Assert
    // Si la barrière a bien fonctionné, le handler a court-circuité avant de réévaluer
    // la suppression dans ScyllaDB qui aurait levé une erreur s'il avait retenté de l'idempotence interne.
    assert!(f.idempotency_repo().exists(None, &command_id).await?);

    Ok(())
}

#[tokio::test]
async fn test_remove_post_from_map_handler_sharding_violation() -> Result<()> {
    // Arrange
    let f = GeoDiscoveryTestFixture::new();
    let command_id = Uuid::now_v7();
    let profile_id = ProfileId::generate();

    let invalid_region = Region::from_str("US").unwrap();
    assert_ne!(f.region(), invalid_region);

    let remove_cmd = RemovePostFromMapCommand {
        command_id,
        target: CommandTarget::stateless(profile_id, invalid_region),
        post_id: PostId::generate(),
        location: GeoPoint::try_new(48.8156, 2.3204).unwrap(),
        created_at: Utc::now(),
    };

    // Act
    let result = f
        .bus()
        .execute::<GeoDiscoveryCommandContext, RemovePostFromMapCommand, ()>(
            f.command_ctx().clone(),
            remove_cmd,
        )
        .await;

    // Assert
    assert!(
        result.is_err(),
        "La violation de région aurait dû bloquer l'effacement"
    );
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::ValidationFailed);
    assert!(err.message.contains("Validation failed"));

    Ok(())
}
