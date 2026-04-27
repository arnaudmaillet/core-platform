#[cfg(test)]
mod tests {
    use shared_kernel::{
        domain::{
            events::{AggregateMetadata, AggregateRoot},
            value_objects::{AccountId, AuditReason, RegionCode},
        },
        errors::Result,
    };

    use crate::domain::{
        account::entities::Account,
        value_objects::{AccountState, Email, ExternalId, RegistrationIdentifier, TrustScore},
    };

    fn create_test_account() -> Account {
        let id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let external_id = ExternalId::try_new("auth0|123").unwrap();
        let identifier =
            RegistrationIdentifier::from_email(Email::try_new("john@example.com").unwrap());

        Account::builder(id, region, identifier, external_id)
            .build()
            .expect("Failed to build test account")
    }

    #[test]
    fn test_account_initial_state() -> Result<()> {
        let account = create_test_account();

        assert_eq!(account.identity().state(), &AccountState::Pending);
        assert!(!account.identity().is_email_verified());
        // La version est portée par l'agrégat via AggregateMetadata
        assert_eq!(
            account.metadata().version(),
            AggregateMetadata::INITIAL_VERSION
        );
        assert_eq!(account.governance().trust_score().value(), TrustScore::MAX);

        Ok(())
    }

    #[test]
    fn test_email_verification_flow_with_bonus() -> Result<()> {
        let mut account = create_test_account();
        let token = "valid_token";
        let snapshot_version = account.version();
        // Action
        let changed = account.verify_email(token)?;

        assert!(changed);
        assert!(account.identity().is_email_verified());
        assert_eq!(account.identity().state(), &AccountState::Active);

        // Vérification du bonus de confiance automatique
        // Score initial (100) + Bonus (10) plafonné à MAX (100)
        // Pour tester le reward, on pourrait baisser le score avant
        assert_eq!(account.governance().trust_score().value(), TrustScore::MAX);

        // Vérification de la version globale
        assert_eq!(account.metadata().version(), snapshot_version + 1);

        Ok(())
    }

    #[test]
    fn test_account_suspension_and_unsuspend_with_reason() -> Result<()> {
        let mut account = create_test_account();
        let snapshot_version = account.version();

        // Suspension avec raison obligatoire (&str)
        let changed = account.suspend(AuditReason::try_new("Suspicious activity")?)?;
        assert!(changed);
        assert!(account.identity().is_blocked());
        assert_eq!(account.metadata().version(), 2);

        // Unsuspend avec raison optionnelle (Option<&str>)
        let changed = account.unsuspend(AuditReason::try_new("Cleared by support")?)?;
        assert!(changed);
        assert!(account.identity().is_active());
        assert_eq!(account.metadata().version(), snapshot_version + 2);

        Ok(())
    }

    #[test]
    fn test_banning_and_trust_score_destruction() -> Result<()> {
        let mut account = create_test_account();

        // Ban (Raison obligatoire)
        account.ban(AuditReason::try_new("Violation of TOS")?)?;

        assert_eq!(account.identity().state(), &AccountState::Banned);
        // Le ban doit détruire le trust score (Penalty 100)
        assert_eq!(account.governance().trust_score().value(), TrustScore::MIN);

        // Unban (Raison optionnelle)
        account.unban(AuditReason::system("Automatic unban"))?;
        assert_eq!(account.identity().state(), &AccountState::Active);
        // Le unban redonne un petit bonus de réhabilitation (20)
        assert_eq!(account.governance().trust_score().value(), 20);

        Ok(())
    }

    #[test]
    fn test_trust_score_operations() -> Result<()> {
        let mut account = create_test_account();

        // On baisse manuellement le score
        account.penalize_trust(30, AuditReason::try_new("Minor warning")?)?;
        assert_eq!(account.governance().trust_score().value(), 70);

        // On remonte
        account.reward_trust(10, AuditReason::try_new("Good behavior")?)?;
        assert_eq!(account.governance().trust_score().value(), 80);

        Ok(())
    }

    #[test]
    fn test_activity_throttling() -> Result<()> {
        let mut account = create_test_account();

        // Premier enregistrement (toujours true à l'init)
        let first = account.record_activity()?;
        assert!(first);

        // Deuxième immédiat (doit être false cause throttling 5min)
        let second = account.record_activity()?;
        assert!(!second);

        Ok(())
    }

    #[test]
    fn test_shadowban_logic() -> Result<()> {
        let mut account = create_test_account();

        assert!(!account.governance().is_shadowbanned());

        account.shadowban(AuditReason::try_new("Investigation pending")?)?;
        assert!(account.governance().is_shadowbanned());

        account.lift_shadowban(AuditReason::try_new("Investigation cleared")?)?;
        assert!(!account.governance().is_shadowbanned());

        Ok(())
    }
}
