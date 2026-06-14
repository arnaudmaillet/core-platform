// crates/geo_discovery/tests/scylla_persistence_it.rs

use chrono::{Duration, Utc};
use infra_test::ScyllaTestContext;
use std::str::FromStr;
use std::time::Duration as StdDuration;

use geo_discovery::entities::MapAnnotation;
use geo_discovery::repositories::MapAnnotationArchiveRepository;
use geo_discovery::stores::ScyllaMapAnnotationStore;
use geo_discovery::types::{BucketHour, TileH3, TileResolution};

use shared_kernel::core::Result;
use shared_kernel::geo::GeoPoint;
use shared_kernel::types::{PostId, PostType};

/// Initialise le contexte d'infrastructure éphémère ScyllaDB pour le module geo_discovery
async fn get_test_context() -> (ScyllaMapAnnotationStore, ScyllaTestContext) {
    let valid_path = ["./migrations/scylla"]
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .expect("💥 Impossible de localiser le dossier des migrations CQL de geo_discovery");

    let scylla_ctx = ScyllaTestContext::builder()
        .with_keyspace("geo_discovery")
        .with_migrations(&[valid_path])
        .build()
        .await;

    let repo = ScyllaMapAnnotationStore::new(scylla_ctx.session().clone())
        .await
        .expect("Échec de l'initialisation du ScyllaMapPersistenceRepository");

    (repo, scylla_ctx)
}

fn create_fixture_active_post(
    post_id: PostId,
    resolution: TileResolution,
    tile_id: TileH3,
    expires_at: chrono::DateTime<Utc>,
) -> MapAnnotation {
    let location = GeoPoint::try_new(48.8566, 2.3522).unwrap(); // Paris Centre (Sûr)
    MapAnnotation::builder(post_id, location, resolution, tile_id)
        .with_post_type(PostType::Video)
        .with_created_at(Utc::now())
        .with_expires_at(expires_at)
        .build()
        .unwrap()
}

#[tokio::test]
async fn test_active_post_full_lifecycle_and_partition_query() -> Result<()> {
    // Arrange
    let (repo, _scylla_ctx) = get_test_context().await;

    let resolution = TileResolution::try_new(7).unwrap();
    let tile_id = TileH3::from_str("871fb4670ffffff").unwrap();
    let now = Utc::now();
    let bucket = BucketHour::from_timestamp(now.timestamp_millis());

    let post_id = PostId::generate();
    let active_post = create_fixture_active_post(
        post_id,
        resolution,
        tile_id.clone(),
        now + Duration::hours(2),
    );

    // --- Act 1: Sauvegarde ---
    tracing::info!(%post_id, "Writing active map post to ScyllaDB cluster...");
    repo.save(&active_post, StdDuration::from_secs(7200))
        .await?;

    // --- Act 2: Récupération par partition ---
    let posts_in_tile = repo.find_by_tile(resolution, &tile_id, bucket).await?;

    // --- Assert 1: Présence ---
    assert_eq!(
        posts_in_tile.len(),
        1,
        "Le post devrait être indexé sous cette partition"
    );
    let found_post = &posts_in_tile[0];
    assert_eq!(found_post.post_id(), post_id);
    assert_eq!(found_post.resolution(), resolution);
    assert_eq!(found_post.tile_id(), &tile_id);

    // --- Act 3: Suppression ---
    tracing::info!(%post_id, "Deleting active map post from ScyllaDB cluster...");
    repo.delete(resolution, &tile_id, bucket, &post_id).await?;

    // --- Assert 2: Disparition complète ---
    let posts_after_delete = repo.find_by_tile(resolution, &tile_id, bucket).await?;
    assert!(
        posts_after_delete.is_empty(),
        "La partition devrait être vide suite à l'exécution de l'effacement"
    );

    Ok(())
}

#[tokio::test]
async fn test_find_by_tile_handles_clustering_keys_and_filtering() -> Result<()> {
    // Arrange
    let (repo, _scylla_ctx) = get_test_context().await;

    let resolution = TileResolution::try_new(7).unwrap();
    let tile_id = TileH3::from_str("871fb4672ffffff").unwrap();
    let now = Utc::now();
    let bucket = BucketHour::from_timestamp(now.timestamp_millis());

    // Insertion de 3 publications dans la MÊME partition (Même tuile, même heure brute)
    let id_1 = PostId::generate();
    let id_2 = PostId::generate();
    let id_3 = PostId::generate();

    let post_1 =
        create_fixture_active_post(id_1, resolution, tile_id.clone(), now + Duration::hours(1));
    let post_2 =
        create_fixture_active_post(id_2, resolution, tile_id.clone(), now + Duration::hours(2));
    // Post 3 est déjà expiré logiquement au niveau applicatif (expires_at <= now)
    let post_3 = create_fixture_active_post(
        id_3,
        resolution,
        tile_id.clone(),
        now - Duration::minutes(5),
    );

    repo.save(&post_1, StdDuration::from_secs(3600)).await?;
    repo.save(&post_2, StdDuration::from_secs(7200)).await?;
    repo.save(&post_3, StdDuration::from_secs(3600)).await?;

    // --- Act ---
    let active_records = repo.find_by_tile(resolution, &tile_id, bucket).await?;

    // --- Assert ---
    // post_3 doit être écarté à la lecture car sa date d'expiration est inférieure ou égale à Utc::now()
    assert_eq!(
        active_records.len(),
        2,
        "Seuls les posts non expirés applicativement doivent être extraits de la partition"
    );

    let contains_1 = active_records.iter().any(|p| p.post_id() == id_1);
    let contains_2 = active_records.iter().any(|p| p.post_id() == id_2);
    let contains_3 = active_records.iter().any(|p| p.post_id() == id_3);

    assert!(contains_1, "Le Post 1 valide est manquant");
    assert!(contains_2, "Le Post 2 valide est manquant");
    assert!(
        !contains_3,
        "Le Post 3 expiré a fui la clause de filtrage du Repository"
    );

    Ok(())
}

#[tokio::test]
async fn test_find_by_tile_returns_empty_safely_when_no_records_exist() -> Result<()> {
    // Arrange
    let (repo, _scylla_ctx) = get_test_context().await;

    let resolution = TileResolution::try_new(10).unwrap();
    let empty_tile = TileH3::from_str("8af101000000fff").unwrap();
    let bucket = BucketHour::from_timestamp(Utc::now().timestamp_millis());

    // Act
    let result = repo.find_by_tile(resolution, &empty_tile, bucket).await?;

    // Assert
    assert!(
        result.is_empty(),
        "Une partition ScyllaDB inexistante doit renvoyer un Vec vide sans planter"
    );
    Ok(())
}
