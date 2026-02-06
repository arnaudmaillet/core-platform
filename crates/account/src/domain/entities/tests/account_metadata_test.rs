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
    fn test_increase_trust_score_and_clamping() {
        let mut meta = create_test_metadata();
        let region = meta.region_code().clone(); // Libère l'emprunt de meta
        let action_id = Uuid::now_v7();

        // On augmente de 10 -> Passage à 110 (Max 100 si ton code clamp à 100)
        // Si ton code clamp à 100, alors passer de 100 à 100 via un clamp doit renvoyer false
        let changed = meta.increase_trust_score(&region, action_id, 10, "Good behavior".into()).unwrap();

        // Ici, si le score initial est 100 et le max est 100, changed sera false
        assert_eq!(meta.trust_score(), 100);
        assert!(!changed);

        // Test avec une valeur qui change réellement (on baisse d'abord)
        meta.decrease_trust_score(&region, action_id, 20, "Penalty".into()).unwrap();
        let changed = meta.increase_trust_score(&region, action_id, 10, "Bouncing back".into()).unwrap();
        assert!(changed);
        assert_eq!(meta.trust_score(), 90);
    }

    #[test]
    fn test_automated_shadowban_on_low_score() {
        let mut meta = create_test_metadata();
        let region = meta.region_code().clone();
        let action_id = Uuid::now_v7();

        // On baisse le score lourdement
        let changed = meta.decrease_trust_score(&region, action_id, 130, "Spam detected".into()).unwrap();

        assert!(changed);
        assert!(meta.is_shadowbanned());
        assert!(meta.moderation_notes().unwrap().contains("Automated system: Trust score dropped below critical threshold"));

        let events = meta.metadata_mut().pull_events();
        // 2 événements : TrustScoreAdjusted ET ShadowbanStatusChanged
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_shadowban_lifecycle_idempotency() {
        let mut meta = create_test_metadata();
        let region = meta.region_code().clone();

        let changed = meta.shadowban(&region, "Reason".into()).unwrap();
        assert!(changed);
        let events_count = meta.pull_events().len(); // On vide

        let changed_again = meta.shadowban(&region, "Reason".into()).unwrap();
        assert!(!changed_again); // Idempotence
        assert_eq!(meta.pull_events().len(), 0); // Pas de nouvel event

        // Lift : Succès (true)
        let changed = meta.lift_shadowban(&region, "Apologies".into()).unwrap();
        assert!(changed);
        assert!(!meta.is_shadowbanned());
    }

    #[test]
    fn test_role_upgrade_logic() {
        let mut meta = create_test_metadata();
        let region = meta.region_code().clone();

        // Upgrade vers Staff : Succès
        let changed = meta.upgrade_role(&region, AccountRole::Staff, "Promoted".into()).unwrap();
        assert!(changed);
        assert!(meta.is_staff());

        // Idempotence : upgrade vers le même rôle -> false
        let changed = meta.upgrade_role(&region, AccountRole::Staff, "Again".into()).unwrap();
        assert!(!changed);
    }

    #[test]
    fn test_cross_region_security_guard() {
        let mut meta = create_test_metadata(); // Initialisé en "eu"
        let wrong_region = RegionCode::try_new("us").unwrap();

        // Doit renvoyer une erreur Forbidden et non un booléen false
        let result = meta.upgrade_role(&wrong_region, AccountRole::Staff, "Hack".into());
        assert!(result.is_err());
    }

    #[test]
    fn test_beta_status_toggle() {
        let mut meta = create_test_metadata();
        let region = meta.region_code().clone(); // On extrait la région

        // Premier appel : doit renvoyer Ok(true)
        let changed = meta.set_beta_status(&region, true, "Enrolled".into()).unwrap();
        assert!(changed);
        assert!(meta.is_beta_tester());

        // Deuxième appel : doit renvoyer Ok(false)
        let changed = meta.set_beta_status(&region, true, "Enrolled again".into()).unwrap();
        assert!(!changed);
    }

    #[test]
    fn test_change_region() {
        let mut meta = create_test_metadata();
        let new_region = RegionCode::try_new("us").unwrap();

        // Premier changement : true
        let changed = meta.change_region(new_region.clone()).unwrap();
        assert!(changed);
        assert_eq!(meta.region_code(), &new_region);

        // Idempotence : même région -> false
        let changed = meta.change_region(new_region).unwrap();
        assert!(!changed);
    }

    #[test]
    fn test_trust_levels() {
        let mut meta = create_test_metadata(); // score 100
        let region = meta.region_code().clone();

        // On baisse à 50 pour être sûr de tester la remontée
        meta.decrease_trust_score(&region, Uuid::now_v7(), 50, "Reset".into()).unwrap();
        assert!(!meta.is_high_trust());

        // On remonte à 101 (si ton code autorise > 100) ou on teste le seuil
        meta.increase_trust_score(&region, Uuid::now_v7(), 51, "Bump".into()).unwrap();
        // Note: Ajuste cette assertion selon ta règle métier is_high_trust
        // assert!(meta.is_high_trust());
    }
}