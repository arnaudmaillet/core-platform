#[cfg(test)]
mod tests {
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::value_objects::*;
    use chrono::{Duration, Utc};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;

    // Helper pour créer un compte de base rapidement
    fn create_test_account() -> AccountIdentity {
        let id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let email = Email::try_new("john@example.com").unwrap();
        let external_id = ExternalId::try_new("auth0|123").unwrap();

        AccountIdentity::builder(id, region, email, external_id)
            .with_last_active_at(Utc::now() - Duration::hours(1))
            .build()
    }

    #[test]
    fn test_account_initial_state() {
        let account = create_test_account();

        assert_eq!(account.state(), &AccountState::Pending);
        assert!(!account.is_email_verified());
        assert_eq!(account.version(), 1);
    }

    #[test]
    fn test_email_verification_flow_and_idempotency() {
        let mut account = create_test_account();

        // 1. Plus de paramètre &region
        let changed = account.verify_email().expect("Should verify email");
        assert!(changed);
        assert!(account.is_email_verified());
        assert_eq!(account.state(), &AccountState::Active);

        let _ = account.metadata_mut().pull_events();

        // 2. Idempotence simple
        let changed = account.verify_email().unwrap();
        assert!(!changed);
    }

    #[test]
    fn test_identity_linking_security() {
        let mut account = create_test_account();

        // Liaison identique (Idempotence)
        let same_id = ExternalId::try_new("auth0|123").unwrap();
        let changed = account.link_external_identity(same_id).unwrap();
        assert!(!changed);

        // Tentative de changement d'identité externe (Règle métier : Interdit)
        let new_id = ExternalId::try_new("google|456").unwrap();
        let result = account.link_external_identity(new_id);
        assert!(result.is_err(), "Should not allow re-linking");
    }

    #[test]
    fn test_account_suspension_lifecycle() {
        let mut account = create_test_account();
        account.verify_email().unwrap();

        // Suspension
        let changed = account.suspend("Suspicious activity".into()).unwrap();
        assert!(changed);
        assert!(account.is_blocked());

        // Unsuspend
        let changed = account.unsuspend().unwrap();
        assert!(changed);
        assert!(account.is_active());
    }

    #[test]
    fn test_banning_constraints() {
        let mut account = create_test_account();

        account.ban("Violation of TOS".into()).unwrap();
        assert_eq!(account.state(), &AccountState::Banned);

        // Règle métier : On ne peut pas "activer" un banni sans l' "unban"
        let res = account.activate();
        assert!(res.is_err());

        account.unban().unwrap();
        assert_eq!(account.state(), &AccountState::Active);
    }

    #[test]
    fn test_activity_recording_throttling() {
        let mut account = create_test_account();

        // Premier log : True (car l'heure dans le builder est ancienne)
        let first_log = account.record_activity().unwrap();
        assert!(first_log);

        // Second log : False (Throttling < 5 mins)
        let second_log = account.record_activity().unwrap();
        assert!(!second_log);
    }
}
