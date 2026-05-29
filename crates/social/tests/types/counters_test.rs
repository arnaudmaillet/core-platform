
use chrono::{Duration, Utc};
use shared_kernel::core::{AggregateRoot, Entity, Result, Versioned};
use shared_kernel::types::{Counter, ProfileId};
use social::entities::ProfileCounters;
use uuid::Uuid;

// Helper pour générer un ProfileId
fn create_mock_profile_id() -> ProfileId {
    ProfileId::from(Uuid::new_v4())
}

#[test]
fn test_should_initialize_new_counters_at_zero() -> Result<()> {
    // Given
    let profile_id = create_mock_profile_id();

    // When
    let counters = ProfileCounters::new(profile_id);

    // Then
    assert_eq!(counters.profile_id(), &profile_id);
    assert_eq!(counters.followers_count().value(), 0);
    assert_eq!(counters.following_count().value(), 0);
    assert_eq!(counters.version(), 0);

    // Validation des traits du Kernel
    assert_eq!(Entity::id(&counters), &profile_id);
    assert_eq!(AggregateRoot::id(&counters), profile_id.to_string());
    assert_eq!(ProfileCounters::entity_name(), "ProfileCounters");

    Ok(())
}

#[test]
fn test_should_restore_counters_from_infrastructure_state() -> Result<()> {
    // Given
    let profile_id = create_mock_profile_id();
    let expected_followers = Counter::from_raw(1250);
    let expected_following = Counter::from_raw(420);
    let expected_version = 8;
    let created_at = Utc::now() - Duration::hours(6);
    let past_update = Utc::now() - Duration::hours(4);

    // When
    let counters = ProfileCounters::restore(
        profile_id,
        expected_followers,
        expected_following,
        expected_version,
        created_at,
        past_update,
    );

    // Then
    assert_eq!(counters.profile_id(), &profile_id);
    assert_eq!(counters.followers_count().value(), 1250);
    assert_eq!(counters.following_count().value(), 420);
    assert_eq!(counters.version(), expected_version);
    assert_eq!(Versioned::updated_at(&counters), past_update);

    Ok(())
}

#[test]
fn test_apply_follower_change_should_increment_and_record_change() -> Result<()> {
    // Given
    let mut counters = ProfileCounters::new(create_mock_profile_id());
    let initial_version = counters.version();
    let initial_update_time = Versioned::updated_at(&counters);

    // When
    let result = counters.apply_follower_change(true)?; // true = increment

    // Then
    assert!(result);
    assert_eq!(counters.followers_count().value(), 1);
    assert_eq!(counters.following_count().value(), 0); // Invariant : le suivant ne bouge pas
    assert_eq!(counters.version(), initial_version + 1);
    assert!(Versioned::updated_at(&counters) >= initial_update_time);

    Ok(())
}

#[test]
fn test_apply_follower_change_should_decrement_and_record_change() -> Result<()> {
    // Given
    // On restaure un profil qui a déjà 10 followers
    let mut counters = ProfileCounters::restore(
        create_mock_profile_id(),
        Counter::from_raw(10),
        Counter::from_raw(5),
        1,
        Utc::now(),
        Utc::now(),
    );

    // When
    let result = counters.apply_follower_change(false)?; // false = decrement

    // Then
    assert!(result);
    assert_eq!(counters.followers_count().value(), 9);
    assert_eq!(counters.following_count().value(), 5);
    assert_eq!(counters.version(), 2);

    Ok(())
}

#[test]
fn test_apply_following_change_should_increment_and_record_change() -> Result<()> {
    // Given
    let mut counters = ProfileCounters::new(create_mock_profile_id());
    let initial_version = counters.version();

    // When
    let result = counters.apply_following_change(true)?;

    // Then
    assert!(result);
    assert_eq!(counters.following_count().value(), 1);
    assert_eq!(counters.followers_count().value(), 0);
    assert_eq!(counters.version(), initial_version + 1);

    Ok(())
}

#[test]
fn test_apply_following_change_should_decrement_and_record_change() -> Result<()> {
    // Given
    let mut counters = ProfileCounters::restore(
        create_mock_profile_id(),
        Counter::from_raw(50),
        Counter::from_raw(50),
        4,
        Utc::now(),
        Utc::now(),
    );

    // When
    let result = counters.apply_following_change(false)?;

    // Then
    assert!(result);
    assert_eq!(counters.following_count().value(), 49);
    assert_eq!(counters.followers_count().value(), 50);
    assert_eq!(counters.version(), 5);

    Ok(())
}

#[test]
fn test_map_constraint_to_field_should_match_scylla_indexes() -> Result<()> {
    assert_eq!(
        ProfileCounters::map_constraint_to_field("profile_counters_pkey"),
        "profile_id"
    );
    assert_eq!(
        ProfileCounters::map_constraint_to_field("random_db_err"),
        "internal_governance"
    );

    Ok(())
}
