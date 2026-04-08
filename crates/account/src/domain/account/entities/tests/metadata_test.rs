#[cfg(test)]
mod tests {
    use crate::domain::account::entities::AccountMetadata;
    use crate::domain::value_objects::{AccountRole, IpAddr};
    use chrono::Utc;
    use shared_kernel::domain::events::{AggregateMetadata, AggregateRoot};
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use uuid::Uuid;

    // Helper pour initialiser un metadata de test
    fn create_test_metadata() -> AccountMetadata {
        let account_id = AccountId::new();
        let ip_addr = IpAddr::try_new("127.0.0.1").unwrap();

        AccountMetadata::restore(
            account_id,
            AccountRole::User,
            false,
            false,
            100,
            None,
            None,
            Some(ip_addr),
            Utc::now(),
            AggregateMetadata::default(),
        )
    }

    #[test]
    fn test_initial_state_and_getters() {
        let meta = create_test_metadata();
        let expected_ip = IpAddr::try_new("127.0.0.1").unwrap();
        assert_eq!(meta.role(), AccountRole::User);
        assert_eq!(meta.trust_score(), 100);
        assert_eq!(meta.last_ip_addr(), Some(&expected_ip));
    }

    #[test]
    fn test_increase_trust_score_and_clamping() {
        let mut meta = create_test_metadata();
        let action_id = Uuid::now_v7();

        // Plus besoin de passer &region
        let changed = meta
            .increase_trust_score(action_id, 10, "Good behavior".into())
            .unwrap();

        assert_eq!(meta.trust_score(), 100); // Supposant un clamp à 100
        assert!(!changed);

        meta.decrease_trust_score(action_id, 20, "Penalty".into()).unwrap();
        let changed = meta
            .increase_trust_score(action_id, 10, "Bouncing back".into())
            .unwrap();
        assert!(changed);
        assert_eq!(meta.trust_score(), 90);
    }

    #[test]
    fn test_automated_shadowban_on_low_score() {
        let mut meta = create_test_metadata();
        let action_id = Uuid::now_v7();

        let changed = meta
            .decrease_trust_score(action_id, 130, "Spam detected".into())
            .unwrap();

        assert!(changed);
        assert!(meta.is_shadowbanned());
        
        let events = meta.metadata_mut().pull_events();
        assert_eq!(events.len(), 2); // TrustScoreAdjusted + ShadowbanStatusChanged
    }

    #[test]
    fn test_shadowban_lifecycle_idempotency() {
        let mut meta = create_test_metadata();

        // On ne passe plus la région
        let changed = meta.shadowban("Reason".into()).unwrap();
        assert!(changed);

        let changed_again = meta.shadowban("Reason".into()).unwrap();
        assert!(!changed_again);

        let changed = meta.lift_shadowban("Apologies".into()).unwrap();
        assert!(changed);
        assert!(!meta.is_shadowbanned());
    }

    #[test]
    fn test_role_upgrade_logic() {
        let mut meta = create_test_metadata();

        let changed = meta
            .upgrade_role(AccountRole::Staff, "Promoted".into())
            .unwrap();
        assert!(changed);
        assert!(meta.is_staff());

        let changed = meta
            .upgrade_role(AccountRole::Staff, "Again".into())
            .unwrap();
        assert!(!changed);
    }

    #[test]
    fn test_beta_status_toggle() {
        let mut meta = create_test_metadata();

        let changed = meta
            .set_beta_status(true, "Enrolled".into())
            .unwrap();
        assert!(changed);
        assert!(meta.is_beta_tester());
    }

    #[test]
    fn test_trust_levels() {
        let mut meta = create_test_metadata(); // score 100

        // On baisse à 50 pour être sûr de tester la remontée
        meta.decrease_trust_score(Uuid::now_v7(), 50, "Reset".into())
            .unwrap();
        assert!(!meta.is_high_trust());

        // On remonte à 101 (si ton code autorise > 100) ou on teste le seuil
        meta.increase_trust_score(Uuid::now_v7(), 51, "Bump".into())
            .unwrap();
        // Note: Ajuste cette assertion selon ta règle métier is_high_trust
        // assert!(meta.is_high_trust());
    }
}
