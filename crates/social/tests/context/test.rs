// crates/social/src/application/context/test.rs

use chrono::{Duration, Utc};
use shared_kernel::core::{ErrorCode, Result};
use shared_kernel::types::{ProfileId, Region, RegionCode};
use social::entities::FollowRelation;
use social::entities::FollowRelationBuilder;
use social_test_utils::SocialTestFixture;
use social_test_utils::assertions::{CounterRepositoryAsserts, RelationRepositoryAsserts};

// --- TESTS DU BUILDER ---

#[test]
fn test_builder_should_instantiate_relation_correctly() -> Result<()> {
    // Given
    let follower = ProfileId::generate();
    let following = ProfileId::generate();
    let custom_time = Utc::now() - Duration::days(2);

    // When
    let relation = FollowRelationBuilder::new(follower, following)
        .with_created_at(custom_time)
        .build()?;

    // Then
    assert_eq!(relation.follower_id(), follower);
    assert_eq!(relation.following_id(), following);
    assert_eq!(relation.created_at(), custom_time);
    Ok(())
}

#[tokio::test]
async fn test_get_counters_cache_hit_should_return_immediately() -> Result<()> {
    // Given
    let fixture = SocialTestFixture::new();
    let profile_id = fixture.target_profile_id();

    // On alimente uniquement le cache chaud (Redis)
    fixture
        .cache_counter_repo()
        .seed_counters(profile_id, 100, 50);

    // When
    let counters = fixture.query_ctx().get_profile_counters(profile_id).await?;

    // Then
    assert_eq!(counters.followers_count().value(), 100);
    assert_eq!(counters.following_count().value(), 50);
    Ok(())
}

#[tokio::test]
async fn test_get_counters_cache_miss_should_fallback_and_warm_cache() -> Result<()> {
    // Given
    let fixture = SocialTestFixture::new();
    let profile_id = fixture.target_profile_id();

    // Donnée présente uniquement dans la DB consolidée (ScyllaDB)
    fixture
        .db_counter_repo()
        .seed_counters(profile_id, 1250, 420);

    // When
    let counters = fixture.query_ctx().get_profile_counters(profile_id).await?;

    // Then
    assert_eq!(counters.followers_count().value(), 1250);
    assert_eq!(counters.following_count().value(), 420);

    // Verification du CACHE WARMING : Redis doit maintenant avoir la donnée via notre méthode d'assertion
    fixture
        .cache_counter_repo()
        .assert_counters_values(profile_id, 1250, 420)
        .await;
    Ok(())
}

#[tokio::test]
async fn test_ensure_executable_should_fail_on_region_mismatch() -> Result<()> {
    // Given
    let fixture = SocialTestFixture::new();

    let context_region_code = fixture.region().inner();
    let wrong_region_code = match context_region_code {
        RegionCode::EU => RegionCode::US,
        _ => RegionCode::EU,
    };

    let wrong_region = Region::from_raw(wrong_region_code);

    // When
    let result = fixture.command_ctx().ensure_executable(&wrong_region).await;

    // Then
    assert!(
        result.is_err(),
        "L'exécution aurait dû être bloquée pour cause de mismatch de région"
    );
    let error = result.unwrap_err();

    assert_eq!(error.code, ErrorCode::ValidationFailed);
    assert!(
        error.message.contains("region"),
        "Le message aurait dû cibler le champ 'region', reçu: '{}'",
        error.message
    );
    Ok(())
}

#[tokio::test]
async fn test_save_relation_should_execute_gpc_flow_synchronously() -> Result<()> {
    // Given
    let fixture = SocialTestFixture::new();

    let follower_id = fixture.target_profile_id();
    let following_id = ProfileId::generate();

    let mut relation = FollowRelation::builder(follower_id, following_id).build()?;

    // When
    fixture.command_ctx().save_relation(&mut relation).await?;

    // Then
    // 1. Vérification de l'écriture Graphe synchrone (ScyllaDB)
    fixture
        .relation_repo()
        .assert_relation_exists(follower_id, following_id)
        .await;

    // 2. Vérification du Hot Path Compteurs & Marquage Dirty dans le cache Redis
    fixture
        .cache_counter_repo()
        .assert_counters_values(follower_id, 0, 1)
        .await; // Il suit quelqu'un (+1 following)
    fixture
        .cache_counter_repo()
        .assert_counters_values(following_id, 1, 0)
        .await; // L'autre gagne un follower (+1 follower)
    fixture
        .cache_counter_repo()
        .assert_profile_is_dirty(&follower_id);

    Ok(())
}

#[tokio::test]
async fn test_delete_relation_should_execute_unfollow_flow_synchronously() -> Result<()> {
    // Given
    let fixture = SocialTestFixture::new();

    let follower_id = fixture.target_profile_id();
    let following_id = ProfileId::generate();

    // État initial (Given): La relation existe et les compteurs sont déjà chauds
    fixture.given_existing_relation(follower_id, following_id);
    fixture.given_initial_counters(follower_id, 0, 1);
    fixture.given_initial_counters(following_id, 1, 0);

    let mut relation = FollowRelation::builder(follower_id, following_id).build()?;

    // When
    fixture.command_ctx().delete_relation(&mut relation).await?;

    // Then
    // 1. Retrait immédiat du graphe (ScyllaDB)
    fixture
        .relation_repo()
        .assert_relation_does_not_exist(follower_id, following_id)
        .await;

    // 2. Décrémentation atomique Redis & marquage Dirty
    fixture
        .cache_counter_repo()
        .assert_counters_values(follower_id, 0, 0)
        .await;
    fixture
        .cache_counter_repo()
        .assert_counters_values(following_id, 0, 0)
        .await;

    Ok(())
}
