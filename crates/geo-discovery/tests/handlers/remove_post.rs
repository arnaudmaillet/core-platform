use chrono::{Duration, Utc};
use geo_discovery::repositories::MapAnnotationDiscoveryRepository;
use h3o::{LatLng, Resolution};
use std::str::FromStr;
use uuid::Uuid;

use geo_discovery::context::GeoDiscoveryCommandCtx;
use geo_discovery::types::{PopularityScore, TileH3, TileResolution};
use geo_discovery::use_cases::{IndexMapAnnotationCommand, RemoveMapAnnotationCommand};
use geo_discovery_test_utils::GeoDiscoveryTestFixture;

use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result};
use shared_kernel::geo::GeoPoint;
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::types::{PostId, ProfileId, Region};

#[tokio::test]
async fn test_remove_post_from_map_handler_success() -> Result<()> {
    // Arrange
    let f = GeoDiscoveryTestFixture::new().await;
    let post_id = PostId::generate();
    let profile_id = ProfileId::generate();

    let lat = 48.8566;
    let lon = 2.3522;
    let location = GeoPoint::try_new(lat, lon).unwrap();
    let created_at = Utc::now();

    // On s'assure que le stub ScyllaDB est vide au départ
    assert_eq!(f.map_repo().count_all(), 0);

    // 1. GIVEN : On indexe d'abord le post pour qu'il existe dans nos stubs d'infra
    let index_cmd = IndexMapAnnotationCommand {
        command_id: Uuid::now_v7(),
        target: CommandTarget::stateless(profile_id),
        region: f.region(),
        post_id,
        location,
        post_type: "video".to_string(),
        thumbnail_url: None,
        created_at,
        expires_at: created_at + Duration::days(10), // Expiration lointaine
        popularity_score: PopularityScore::from_raw(10.0),
    };
    f.bus()
        .execute::<GeoDiscoveryCommandCtx, IndexMapAnnotationCommand, ()>(
            f.command_ctx().clone(),
            index_cmd,
        )
        .await?;

    // 💎 ASSERTION GIVEN ULTRA-FIABLE : On utilise persistence_repo().count_all()
    assert_eq!(
        f.map_repo().count_all(),
        1,
        "Le post aurait dû être indexé correctement au départ dans le stub ScyllaDB"
    );

    // 2. WHEN : On exécute le handler de suppression
    let remove_command_id = Uuid::now_v7();
    let remove_cmd = RemoveMapAnnotationCommand {
        command_id: remove_command_id,
        target: CommandTarget::stateless(profile_id),
        region: f.region(),
        post_id,
        location,
        created_at,
    };

    f.bus()
        .execute::<GeoDiscoveryCommandCtx, RemoveMapAnnotationCommand, ()>(
            f.command_ctx().clone(),
            remove_cmd,
        )
        .await?;

    // 3. ASSERT : Vérification de la suppression complète dans ScyllaDB
    // Si le post est supprimé, le count_all doit retomber à 0
    assert_eq!(
        f.map_repo().count_all(),
        0,
        "Le post n'a pas été supprimé de la persistance ScyllaDB"
    );

    // Vérification du nettoyage complet de tous les index de résolutions Redis
    let h3_lat_lng = LatLng::new(lat.to_radians(), lon.to_radians()).unwrap();
    let expected_resolutions = vec![3, 5, 7, 9, 10];

    for res_val in expected_resolutions {
        let res = TileResolution::try_new(res_val).unwrap();
        let h3_res = Resolution::try_from(res_val as u8).unwrap();
        let cell = h3_lat_lng.to_cell(h3_res);
        let tile_id = TileH3::from_str(&cell.to_string()).unwrap();

        let cached_posts = f.map_cache_repo().get_top_posts(res, &tile_id, 50).await?;
        assert_eq!(
            cached_posts.len(),
            0,
            "Le cache Redis niveau {} contient encore des données résiduelles",
            res_val
        );
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
    let f = GeoDiscoveryTestFixture::new().await;
    let command_id = Uuid::now_v7();
    let post_id = PostId::generate();
    let profile_id = ProfileId::generate();
    let location = GeoPoint::try_new(48.8156, 2.3204).unwrap();
    let created_at = Utc::now();

    // On injecte déjà la commande de suppression dans l'idempotence (doublon réseau)
    f.idempotency_repo().save(None, &command_id).await?;

    let remove_cmd = RemoveMapAnnotationCommand {
        command_id,
        target: CommandTarget::stateless(profile_id),
        region: f.region(),
        post_id,
        location,
        created_at,
    };

    // Act
    f.bus()
        .execute::<GeoDiscoveryCommandCtx, RemoveMapAnnotationCommand, ()>(
            f.command_ctx().clone(),
            remove_cmd,
        )
        .await?;

    // Assert
    assert!(f.idempotency_repo().exists(None, &command_id).await?);

    Ok(())
}

#[tokio::test]
async fn test_remove_post_from_map_handler_sharding_violation() -> Result<()> {
    // Arrange
    let f = GeoDiscoveryTestFixture::new().await;
    let command_id = Uuid::now_v7();
    let profile_id = ProfileId::generate();

    let invalid_region = Region::from_str("US").unwrap();
    assert_ne!(f.region(), invalid_region);

    let remove_cmd = RemoveMapAnnotationCommand {
        command_id,
        target: CommandTarget::stateless(profile_id),
        region: invalid_region,
        post_id: PostId::generate(),
        location: GeoPoint::try_new(48.8156, 2.3204).unwrap(),
        created_at: Utc::now(),
    };

    // Act
    let result = f
        .bus()
        .execute::<GeoDiscoveryCommandCtx, RemoveMapAnnotationCommand, ()>(
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
