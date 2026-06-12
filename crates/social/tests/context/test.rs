// crates/social/src/application/context/test.rs

use chrono::{Duration, Utc};
use shared_kernel::core::{ErrorCode, Result};
use shared_kernel::types::ProfileId;
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

// --- TESTS DES COMPTEURS (QUERY FLOWS) ---

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

// --- TESTS DE VÉRIFICATION & SÉCURITÉ DU CONTEXTE ---

#[tokio::test]
async fn test_verify_actors_should_fail_on_target_mismatch() -> Result<()> {
    // Given
    let fixture = SocialTestFixture::new();
    let follower_id = ProfileId::generate();
    let malicious_target_id = ProfileId::generate();

    // When
    let result = fixture
        .command_ctx()
        .verify_actors(follower_id, malicious_target_id);

    // Then
    assert!(result.is_err(), "La validation aurait dû échouer");
    let error = result.unwrap_err();
    assert_eq!(error.code, ErrorCode::ValidationFailed);

    assert!(
        error.message.contains("target"),
        "Le message reçu était: '{}'",
        error.message
    );

    Ok(())
}

#[tokio::test]
async fn test_save_relation_should_fail_on_identity_mismatch_violation() -> Result<()> {
    // Given
    let fixture = SocialTestFixture::new();
    let follower_id = ProfileId::generate();
    let untrusted_following_id = ProfileId::generate();

    let mut relation = FollowRelation::builder(follower_id, untrusted_following_id).build()?;

    // When
    let result = fixture.command_ctx().save_relation(&mut relation).await;

    // Then
    assert!(result.is_err(), "Le contexte aurait dû refuser");
    let error = result.unwrap_err();
    assert_eq!(error.code, ErrorCode::ValidationFailed);

    assert!(
        error.message.contains("following_id"),
        "Le message reçu était: '{}'",
        error.message
    );

    Ok(())
}

// --- TESTS DE PERSISTANCE SYNCHRONE ---

#[tokio::test]
async fn test_save_relation_should_execute_gpc_flow_synchronously() -> Result<()> {
    // Given
    let fixture = SocialTestFixture::new();

    let follower_id = ProfileId::generate();
    let target_id = fixture.target_profile_id(); // Doit être l'identité du contexte

    let mut relation = FollowRelation::builder(follower_id, target_id).build()?;

    // When
    fixture.command_ctx().save_relation(&mut relation).await?;

    // Then
    // 1. Vérification de l'écriture Graphe synchrone (ScyllaDB)
    fixture
        .relation_repo()
        .assert_relation_exists(follower_id, target_id)
        .await;

    // 2. Vérification du Hot Path Compteurs & Marquage Dirty dans le cache Redis
    fixture
        .cache_counter_repo()
        .assert_counters_values(follower_id, 0, 1)
        .await; // Il suit quelqu'un (+1 following)
    fixture
        .cache_counter_repo()
        .assert_counters_values(target_id, 1, 0)
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

    let follower_id = ProfileId::generate();
    let target_id = fixture.target_profile_id(); // Doit être l'identité du contexte

    // État initial (Given): La relation existe et les compteurs sont déjà chauds
    fixture.given_existing_relation(follower_id, target_id);
    fixture.given_initial_counters(follower_id, 0, 1);
    fixture.given_initial_counters(target_id, 1, 0);

    let mut relation = FollowRelation::builder(follower_id, target_id).build()?;

    // When
    fixture.command_ctx().delete_relation(&mut relation).await?;

    // Then
    // 1. Retrait immédiat du graphe (ScyllaDB)
    fixture
        .relation_repo()
        .assert_relation_does_not_exist(follower_id, target_id)
        .await;

    // 2. Décrémentation atomique Redis & marquage Dirty
    fixture
        .cache_counter_repo()
        .assert_counters_values(follower_id, 0, 0)
        .await;
    fixture
        .cache_counter_repo()
        .assert_counters_values(target_id, 0, 0)
        .await;

    Ok(())
}
