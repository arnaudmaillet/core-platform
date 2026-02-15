// crates/profile/tests/infrastructure/repository_it_for_user_location.rs

use profile::domain::builders::UserLocationBuilder;
use profile::domain::repositories::LocationRepository;
use profile::domain::value_objects::ProfileId;
use profile::infrastructure::postgres::repositories::PostgresLocationRepository;
use shared_kernel::domain::entities::GeoPoint;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::value_objects::RegionCode;
use shared_kernel::infrastructure::postgres::utils::PostgresTestContext;

async fn setup_context() -> (PostgresLocationRepository, PostgresTestContext) {
    let ctx = PostgresTestContext::builder()
        .with_migrations(&["./migrations/postgres"])
        .build()
        .await;

    let repo = PostgresLocationRepository::new(ctx.pool());
    (repo, ctx)
}

#[tokio::test]
async fn test_location_upsert_lifecycle() {
    let (repo, _ctx) = setup_context().await;
    let profile_id = ProfileId::new();
    let region = RegionCode::try_new("eu".to_string()).unwrap();
    let point_a = GeoPoint::try_new(2.3522, 48.8566).unwrap();

    let loc = UserLocationBuilder::new(profile_id.clone(), region.clone(), point_a).build();
    repo.save(&loc, None).await.expect("Save failed");

    let mut fetched = repo
        .fetch(&profile_id, &region)
        .await
        .unwrap()
        .expect("Should exist");

    let point_b = GeoPoint::try_new(2.3500, 48.8500).unwrap();
    fetched.update_position(point_b, None, None);

    repo.save(&fetched, None).await.expect("Update failed");

    let final_check = repo
        .fetch(&profile_id, &region)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(final_check.version(), 2);
}

#[tokio::test]
async fn test_find_nearby_users() {
    let (repo, _ctx) = setup_context().await;
    let region = RegionCode::try_new("eu".to_string()).unwrap();

    let user_paris = UserLocationBuilder::new(
        ProfileId::new(),
        region.clone(),
        GeoPoint::try_new(2.3522, 48.8566).unwrap(),
    ).build();
    let user_boulogne = UserLocationBuilder::new(
        ProfileId::new(),
        region.clone(),
        GeoPoint::try_new(2.2433, 48.8397).unwrap(),
    ).build();

    repo.save(&user_paris, None).await.unwrap();
    repo.save(&user_boulogne, None).await.unwrap();

    let center = GeoPoint::try_new(2.3522, 48.8566).unwrap();
    let results = repo
        .fetch_nearby(center, region, 15_000.0, 10)
        .await
        .unwrap();

    assert_eq!(results.len(), 2, "On devrait trouver Paris et Boulogne");
}

#[tokio::test]
async fn test_ghost_mode_is_excluded_from_nearby() {
    let (repo, _ctx) = setup_context().await;
    let region = RegionCode::try_new("eu".to_string()).unwrap();
    let point = GeoPoint::try_new(2.3522, 48.8566).unwrap();

    let mut ghost_user = UserLocationBuilder::new(ProfileId::new(), region.clone(), point).build();
    ghost_user.set_ghost_mode(true);

    repo.save(&ghost_user, None).await.unwrap();

    let results = repo.fetch_nearby(point, region, 1000.0, 10).await.unwrap();
    assert_eq!(results.len(), 0);
}

#[tokio::test]
async fn test_find_nearby_edge_of_radius() {
    let (repo, _ctx) = setup_context().await;

    let region = RegionCode::try_new("eu".to_string()).unwrap();
    let center = GeoPoint::try_new(2.3522, 48.8566).unwrap(); // Paris

    // On utilise un décalage plus petit pour être sûr de tomber entre 10km et 11km
    // À Paris, 0.13 degré de longitude est environ égal à 9.5km.
    // On va placer le point à 0.145 degré, ce qui devrait être autour de 10.5km.
    let just_outside = GeoPoint::try_new(2.3522 + 0.145, 48.8566).unwrap();
    let loc = UserLocationBuilder::new(ProfileId::new(), region.clone(), just_outside).build();
    repo.save(&loc, None).await.unwrap();

    // 1. Recherche à 9km : trop court
    let results_9k = repo
        .fetch_nearby(center, region.clone(), 9_000.0, 10)
        .await
        .unwrap();
    assert_eq!(results_9k.len(), 0, "Le point à 10.5km ne devrait pas être trouvé dans un rayon de 9km");

    // 2. Recherche à 12km : doit le trouver
    let results_12k = repo
        .fetch_nearby(center, region, 12_000.0, 10)
        .await
        .unwrap();

    assert_eq!(
        results_12k.len(),
        1,
        "L'utilisateur devrait être trouvé dans un rayon de 12km"
    );
}

#[tokio::test]
async fn test_regional_isolation_nearby() {
    let (repo, _ctx) = setup_context().await;
    let region_eu = RegionCode::try_new("eu".to_string()).unwrap();
    let region_us = RegionCode::try_new("us".to_string()).unwrap();
    let point = GeoPoint::try_new(2.3522, 48.8566).unwrap();

    let user_eu = UserLocationBuilder::new(ProfileId::new(), region_eu.clone(), point).build();
    let user_us = UserLocationBuilder::new(ProfileId::new(), region_us.clone(), point).build();

    repo.save(&user_eu, None).await.unwrap();
    repo.save(&user_us, None).await.unwrap();

    let results = repo
        .fetch_nearby(point, region_eu, 1000.0, 10)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0.region_code().as_str(), "eu");
}