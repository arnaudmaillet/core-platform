#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::*;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
    use chrono::{Duration, Utc};
    use shared_kernel::domain::events::{AggregateMetadata, AggregateRoot};
    use shared_kernel::errors::DomainError;
    use crate::domain::entities::Account;

    // Helper pour créer un compte de base rapidement
    fn create_test_account() -> Account {
        let id = AccountId::new();
        let region = RegionCode::try_new("eu").expect("Failed to create region_code");
        let username = Username::try_new("john_doe").unwrap();
        let email = Email::try_new("john@example.com").unwrap();
        let external_id = ExternalId::try_new("auth0|123").unwrap();

        Account::builder(id, region, username, email, external_id)
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
        let region = account.region_code().clone();

        // 1. Première vérification : true
        let changed = account.verify_email(&region).expect("Should verify email");
        assert!(changed);
        assert!(account.is_email_verified());
        // L'état Active est déclenché par la vérification d'email
        assert_eq!(account.state(), &AccountState::Active);

        // On nettoie les événements pour tester l'idempotence proprement
        let _ = account.metadata_mut().pull_events();

        // 2. Deuxième vérification : false (idempotence)
        let changed = account.verify_email(&region).unwrap();
        assert!(!changed, "Email already verified, should return false");
        assert_eq!(account.metadata_mut().pull_events().len(), 0);
    }

    #[test]
    fn test_cross_region_security_on_account() {
        let mut account = create_test_account();
        let wrong_region = RegionCode::try_new("us").unwrap();

        // Tentative de vérification d'email avec la mauvaise région
        let result = account.verify_email(&wrong_region);

        assert!(result.is_err(), "L'opération aurait dû être bloquée (Forbidden)");
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[test]
    fn test_identity_linking_security() {
        let mut account = create_test_account();
        let region = account.region_code().clone();

        // Cas 1 : Liaison identique (Idempotence) -> Ok(false)
        let same_id = ExternalId::try_new("auth0|123").unwrap();
        let changed = account.link_external_identity(&region, same_id).unwrap();
        assert!(!changed);

        // Cas 2 : Tentative de changement d'identité externe -> Err
        let new_id = ExternalId::try_new("google|456").unwrap();
        let result = account.link_external_identity(&region, new_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_account_suspension_lifecycle() {
        let mut account = create_test_account();
        let region = account.region_code().clone();
        account.verify_email(&region).unwrap();

        // 1. Suspension : true
        let changed = account.suspend(&region, "Suspicious activity".into()).unwrap();
        assert!(changed);
        assert!(account.is_blocked());

        // 2. Suspension déjà active : false
        let changed = account.suspend(&region, "Duplicate call".into()).unwrap();
        assert!(!changed);

        // 3. Unsuspend : true
        let changed = account.unsuspend(&region).unwrap();
        assert!(changed);
        assert!(account.is_active());
    }

    #[test]
    fn test_banning_constraints() {
        let mut account = create_test_account();
        let region = account.region_code().clone();

        // Ban : true
        let changed = account.ban(&region, "Violation of TOS".into()).unwrap();
        assert!(changed);
        assert_eq!(account.state(), &AccountState::Banned);

        // On ne peut pas réactiver (reactivate) un compte banni sans unban
        let res = account.reactivate(&region);
        assert!(res.is_err());

        // Unban : true
        let changed = account.unban(&region).unwrap();
        assert!(changed);
        assert_eq!(account.state(), &AccountState::Active);
    }

    fn test_activity_recording_throttling() {
        let mut account = create_test_account();
        let region = account.region_code().clone();

        // Le premier log devrait maintenant être true car l'activité initiale est ancienne
        let first_log = account.record_activity(&region).unwrap();
        assert!(first_log, "First log after builder should be true if last_activity is old");

        // Le second log est immédiat, donc throttle -> false
        let second_log = account.record_activity(&region).unwrap();
        assert!(!second_log, "Should be throttled and return false on immediate subsequent call");
    }

    #[test]
    fn test_username_change_with_idempotency() {
        let mut account = create_test_account();
        let region = account.region_code().clone();
        let new_name = Username::try_new("new_john").unwrap();

        // 1. Changement réel : true
        let changed = account.change_username(&region, new_name.clone()).unwrap();
        assert!(changed);
        assert_eq!(account.username().as_str(), "new_john");

        // 2. Même nom : false
        let changed = account.change_username(&region, new_name).unwrap();
        assert!(!changed);

        // 3. Bloqué si banni
        account.ban(&region, "Bye".into()).unwrap();
        let res = account.change_username(&region, Username::try_new("hacker").unwrap());
        assert!(res.is_err());
    }
}