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
        // On change "FR" (pays) par "eu" (macro-région supportée)
        let region = RegionCode::try_new("eu").expect("Failed to create region_code");
        let username = Username::try_new("john_doe").unwrap();
        let email = Email::try_new("john@example.com").unwrap();
        let external_id = ExternalId::try_new("auth0|123").unwrap();

        Account::builder(id, region, username, email, external_id)
            .build()
    }

    #[test]
    fn test_account_initial_state() {
        let account = create_test_account();

        assert_eq!(account.state(), &AccountState::Pending); // État par défaut via builder
        assert!(!account.is_email_verified());
        assert!(!account.is_phone_verified());
        assert!(account.birth_date().is_none());
        assert_eq!(account.version(), 1);
    }

    #[test]
    fn test_email_verification_flow() {
        let mut account = create_test_account();

        // 1. On vérifie l'email
        account.verify_email().unwrap();

        assert!(account.is_email_verified());
        assert_eq!(account.state(), &AccountState::Active);

        // 2. On "tire" les événements de l'agrégat
        // Note : metadata_mut() est nécessaire car pull_events vide la liste interne
        let events = account.metadata_mut().pull_events();

        assert_eq!(events.len(), 1, "Un événement EmailVerified aurait dû être capturé");

        // Optionnel : On peut vérifier le type de l'événement si nécessaire
        // let event = &events[0];
        // ...
    }

    #[test]
    fn test_identity_linking_security() {
        let mut account = create_test_account();

        // Cas 1 : Liaison identique (Idempotence)
        let same_id = ExternalId::try_new("auth0|123").unwrap();
        assert!(account.link_external_identity(same_id).is_ok());

        // Cas 2 : Tentative de changement d'identité externe (Interdit en Hyperscale)
        let new_id = ExternalId::try_new("google|456").unwrap();
        let result = account.link_external_identity(new_id);

        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[test]
    fn test_account_suspension_logic() {
        let mut account = create_test_account();
        account.verify_email().unwrap(); // Pass to Active

        assert!(account.can_login());

        // Suspension
        account.suspend("Suspicious activity".into()).unwrap();
        assert!(account.is_blocked());
        assert!(!account.can_login());
        assert!(account.change_username(Username::try_new("hacker").unwrap()).is_err());

        // Unsuspend
        account.unsuspend().unwrap();
        assert!(account.is_active());
        assert!(account.can_login());
    }

    #[test]
    fn test_banning_lifecycle() {
        let mut account = create_test_account();

        account.ban("Violation of TOS".into()).unwrap();
        assert_eq!(account.state(), &AccountState::Banned);

        // On ne peut pas réactiver manuellement un compte banni (doit être unban d'abord)
        let res = account.reactivate();
        assert!(res.is_err());

        account.unban().unwrap();
        assert_eq!(account.state(), &AccountState::Active);
    }

    #[test]
    fn test_activity_recording_throttling() {
        let id = AccountId::new();
        let initial_active = Utc::now() - Duration::minutes(10);

        // On recrée l'objet avec l'état temporel souhaité via restore
        let mut account = Account::restore(
            id,
            RegionCode::try_new("eu").unwrap(),
            ExternalId::try_new("auth0|123").unwrap(),
            Username::try_new("john_doe").unwrap(),
            Email::try_new("john@example.com").unwrap(),
            true,           // email_verified
            None,           // phone_number
            false,
            AccountState::Active,
            None,           // birth_date
            Locale::default(),
            Utc::now(),
            Utc::now(),
            Some(initial_active),
            AggregateMetadata::default(),
        );

        // Premier record : Doit mettre à jour car 10 min > 5 min
        account.record_activity();
        let first_update = account.last_active_at().unwrap();
        assert!(first_update > initial_active);

        // Deuxième record immédiat : Ne doit PAS mettre à jour (throttle 5 min)
        account.record_activity();
        assert_eq!(account.last_active_at().unwrap(), first_update);
    }

    #[test]
    fn test_username_change_constraints() {
        let mut account = create_test_account();
        let new_name = Username::try_new("new_john").unwrap();

        // Changement OK
        account.change_username(new_name.clone()).unwrap();
        assert_eq!(account.username().as_str(), "new_john");
        assert_eq!(account.metadata().version(), 2);

        // Bloqué si suspendu
        account.suspend("Reason".into()).unwrap();
        let res = account.change_username(Username::try_new("another").unwrap());
        assert!(res.is_err());
    }
}