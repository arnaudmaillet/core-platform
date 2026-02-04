#[cfg(test)]
mod tests {
    use chrono::Utc;
    use super::*;
    use crate::domain::value_objects::AccountRole;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::domain::events::{AggregateMetadata, AggregateRoot};
    use uuid::Uuid;
    use crate::domain::entities::AccountMetadata;

    // Helper pour initialiser un metadata de test
    fn create_test_metadata() -> AccountMetadata {
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        AccountMetadata::restore(
            account_id,
            region,
            AccountRole::User,
            false, // beta
            false, // shadowban
            100,   // trust_score
            None,
            None,
            Some("127.0.0.1".to_string()),
            Utc::now(),
            AggregateMetadata::default(),
        )
    }

    #[test]
    fn test_initial_state_and_getters() {
        let meta = create_test_metadata();
        assert_eq!(meta.role(), AccountRole::User);
        assert_eq!(meta.trust_score(), 100);
        assert!(!meta.is_beta_tester());
        assert!(!meta.is_shadowbanned());
        assert_eq!(meta.estimated_ip(), Some("127.0.0.1"));
        assert!(!meta.is_high_trust());
    }

    #[test]
    fn test_increase_trust_score() {
        let mut meta = create_test_metadata();
        let action_id = Uuid::now_v7();

        meta.increase_trust_score(action_id, 50, "Good behavior".into());

        assert_eq!(meta.trust_score(), 150);
        assert!(meta.moderation_notes().unwrap().contains("Score increased by 50"));
        assert!(meta.last_moderation_at().is_some());

        let events = meta.metadata_mut().pull_events();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_automated_shadowban_on_low_score() {
        let mut meta = create_test_metadata();
        let action_id = Uuid::now_v7();

        // On baisse le score de 100 à -21 (seuil critique < -20)
        meta.decrease_trust_score(action_id, 121, "Spam detected".into());

        assert_eq!(meta.trust_score(), -21);
        assert!(meta.is_shadowbanned());
        assert!(meta.moderation_notes().unwrap().contains("Automated system: Trust score critical"));

        let events = meta.metadata_mut().pull_events();
        // 2 événements : TrustScoreAdjusted ET ShadowbanStatusChanged
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_shadowban_lifecycle_idempotency() {
        let mut meta = create_test_metadata();

        // Ban
        meta.shadowban("Manual ban".into());
        assert!(meta.is_shadowbanned());

        // Re-ban (ne doit rien faire de plus)
        let event_count = meta.metadata_mut().pull_events().len();
        meta.shadowban("Manual ban again".into());
        assert_eq!(meta.metadata_mut().pull_events().len(), 0);

        // Lift
        meta.lift_shadowban("Apologies".into());
        assert!(!meta.is_shadowbanned());
    }

    #[test]
    fn test_role_upgrade_logic() {
        let mut meta = create_test_metadata();

        // Upgrade vers Staff
        meta.upgrade_role(AccountRole::Staff, "Promoted".into()).unwrap();
        assert!(meta.is_staff());
        assert_eq!(meta.role(), AccountRole::Staff);

        // Idempotence : upgrade vers le même rôle
        let result = meta.upgrade_role(AccountRole::Staff, "Again".into());
        assert!(result.is_ok());
        assert_eq!(meta.metadata_mut().pull_events().len(), 1); // Seulement le 1er event
    }

    #[test]
    fn test_moderation_notes_accumulation() {
        let mut meta = create_test_metadata();

        meta.increase_trust_score(Uuid::now_v7(), 10, "Note 1".into());
        meta.set_beta_status(true, "Note 2".into());

        let notes = meta.moderation_notes().unwrap();
        let lines: Vec<&str> = notes.split('\n').collect();

        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("Note 1"));
        assert!(lines[1].contains("Note 2"));
    }

    #[test]
    fn test_beta_status_toggle() {
        let mut meta = create_test_metadata();

        meta.set_beta_status(true, "Enrolled".into());
        assert!(meta.is_beta_tester());

        // Idempotence
        meta.set_beta_status(true, "Enrolled again".into());
        let events = meta.metadata_mut().pull_events();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_change_region() {
        let mut meta = create_test_metadata();
        let new_region = RegionCode::try_new("us").unwrap();

        meta.change_region(new_region.clone()).unwrap();
        assert_eq!(meta.region_code(), &new_region);

        // Idempotence
        meta.change_region(new_region).unwrap();
        assert_eq!(meta.metadata_mut().pull_events().len(), 1);
    }

    #[test]
    fn test_trust_levels() {
        let mut meta = create_test_metadata(); // score 100
        assert!(!meta.is_high_trust()); // score > 100 requis

        meta.increase_trust_score(Uuid::now_v7(), 1, "Bump".into());
        assert!(meta.is_high_trust());

        meta.shadowban("Hidden".into());
        assert!(!meta.is_high_trust()); // Même avec score élevé, shadowban annule le high_trust
    }
}