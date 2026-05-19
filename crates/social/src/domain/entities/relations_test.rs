#[cfg(test)]
mod tests {
    use crate::entities::FollowRelation;
    use chrono::{Duration, Utc};
    use shared_kernel::core::{AggregateRoot, Entity, Result, Versioned};
    use shared_kernel::messaging::EventEmitter;
    use shared_kernel::types::ProfileId;
    use uuid::Uuid;

    // Helper pour générer un ProfileId propre à partir d'un UUID classique
    fn create_mock_profile_id() -> ProfileId {
        ProfileId::from(Uuid::new_v4())
    }

    #[test]
    fn test_should_create_follow_relation_via_builder() -> Result<()> {
        // Given
        let follower = create_mock_profile_id();
        let following = create_mock_profile_id();

        // When
        let relation = FollowRelation::builder(follower, following).build()?;

        // Then
        assert_eq!(relation.follower_id(), &follower);
        assert_eq!(relation.following_id(), &following);
        assert_eq!(relation.version(), 1); // Version initiale DDD standard
        assert!(relation.created_at() <= Utc::now());

        // Validation des IDs d'entité et d'agrégat
        assert_eq!(Entity::id(&relation), &follower);
        assert_eq!(
            AggregateRoot::id(&relation),
            format!("{}:{}", follower, following)
        );
        assert_eq!(FollowRelation::entity_name(), "FollowRelation");

        Ok(())
    }

    #[test]
    fn test_should_restore_relation_with_correct_historical_state() -> Result<()> {
        // Given
        let follower = create_mock_profile_id();
        let following = create_mock_profile_id();
        let past_creation = Utc::now() - Duration::days(5);
        let past_update = Utc::now() - Duration::days(1);
        let expected_version = 42;

        let relation = FollowRelation::restore(
            follower,
            following,
            expected_version,
            past_creation,
            past_update,
        );

        assert_eq!(relation.follower_id(), &follower);
        assert_eq!(relation.following_id(), &following);
        assert_eq!(relation.version(), expected_version);
        assert_eq!(relation.created_at(), past_creation);
        assert_eq!(relation.updated_at(), past_update);

        Ok(())
    }

    #[test]
    fn test_execute_follow_should_mutate_state_and_emit_domain_event() -> Result<()> {
        let follower = create_mock_profile_id();
        let following = create_mock_profile_id();
        let mut relation = FollowRelation::builder(follower, following).build()?;

        let initial_version = relation.version();
        let initial_update_time = relation.updated_at();
        let result = relation.execute_follow();

        assert!(result.is_ok());
        assert!(result.unwrap());

        // L'état de versionnement de l'agrégat doit s'incrémenter via record_change/track_change
        assert_eq!(relation.version(), initial_version + 1);
        assert!(relation.updated_at() >= initial_update_time);

        // Extraction et analyse de l'événement émis
        let mut events = relation.pull_events();
        assert_eq!(
            events.len(),
            1,
            "Un unique événement de domaine aurait dû être émis"
        );

        let event = events.pop().unwrap();

        // On vérifie le type de l'événement et son contenu textuel/structurel
        let event_debug = format!("{:?}", event);
        assert!(event_debug.contains("ProfileFollowed"));
        assert!(event_debug.contains(&follower.to_string()));
        assert!(event_debug.contains(&following.to_string()));
        Ok(())
    }

    #[test]
    fn test_execute_unfollow_should_mutate_state_and_emit_domain_event() -> Result<()> {
        let follower = create_mock_profile_id();
        let following = create_mock_profile_id();

        // On simule une relation déjà existante restaurée depuis ScyllaDB
        let mut relation = FollowRelation::restore(
            follower,
            following,
            12,
            Utc::now() - Duration::hours(12),
            Utc::now() - Duration::hours(12),
        );

        let result = relation.execute_unfollow();

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert_eq!(relation.version(), 13); // 12 + 1

        let mut events = relation.pull_events();
        assert_eq!(events.len(), 1);

        let event = events.pop().unwrap();
        let event_debug = format!("{:?}", event);
        assert!(event_debug.contains("ProfileUnfollowed"));
        assert!(event_debug.contains(&follower.to_string()));
        assert!(event_debug.contains(&following.to_string()));

        Ok(())
    }

    #[test]
    fn test_pull_events_should_clear_aggregate_internal_queue() -> Result<()> {
        let mut relation =
            FollowRelation::builder(create_mock_profile_id(), create_mock_profile_id()).build()?;
        let _ = relation.execute_follow();

        let first_pull = relation.pull_events();
        let second_pull = relation.pull_events();

        assert_eq!(
            first_pull.len(),
            1,
            "Le premier pull doit ramasser l'événement"
        );
        assert_eq!(
            second_pull.len(),
            0,
            "Le second pull doit trouver une file vide"
        );

        Ok(())
    }

    #[test]
    fn test_map_constraint_to_field_should_match_scylla_indexes() -> Result<()> {
        // Test de robustesse des contraintes d'infrastructure mappées au domaine
        assert_eq!(
            FollowRelation::map_constraint_to_field("social_relations_pkey"),
            "follower_id_following_id"
        );
        assert_eq!(
            FollowRelation::map_constraint_to_field("unknown_error_db"),
            "internal_governance"
        );

        Ok(())
    }
}
