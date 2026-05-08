#[cfg(test)]
mod tests {
    use shared_kernel::{
        domain::{
            entities::Versioned,
            events::{AggregateMetadata, AggregateRoot},
            value_objects::{AccountId, AuditReason, Email, RegionCode},
        },
        errors::Result,
    };

    use crate::domain::{
        account::entities::Account,
        value_objects::{
            AccountState, RegistrationIdentifier, TrustDelta, TrustScore, VerificationToken,
        },
    };

    fn create_test_account() -> Account {
        let account_id: AccountId = AccountId::generate(RegionCode::default());
        let identifier =
            RegistrationIdentifier::from_email(Email::try_new("john@example.com").unwrap());

        Account::builder(account_id, identifier)
            .build()
            .expect("Failed to build test account")
    }

    #[test]
    fn test_account_initial_state() -> Result<()> {
        let account = create_test_account();

        assert_eq!(account.identity().state(), &AccountState::PENDING);
        assert_eq!(
            account.metadata().version(),
            AggregateMetadata::INITIAL_VERSION
        );
        assert_eq!(account.governance().trust_score().value(), TrustScore::MAX);

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
        assert_eq!(account.metadata().version(), snapshot_version + 1);

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

        assert_eq!(account.identity().state(), &AccountState::BANNED);
        // Le ban doit détruire le trust score (Penalty 100)
        assert_eq!(account.governance().trust_score().value(), TrustScore::MIN);

        // Unban (Raison optionnelle)
        account.unban(AuditReason::system("Automatic unban"))?;
        assert_eq!(account.identity().state(), &AccountState::ACTIVE);
        // Le unban redonne un petit bonus de réhabilitation (20)
        assert_eq!(account.governance().trust_score().value(), 20);

        Ok(())
    }

    #[test]
    fn test_trust_score_operations() -> Result<()> {
        let mut account = create_test_account();

        // On baisse manuellement le score
        account.penalize_trust(
            TrustDelta::from_raw(30),
            AuditReason::try_new("Minor warning")?,
        )?;
        assert_eq!(account.governance().trust_score().value(), 70);

        // On remonte
        account.reward_trust(
            TrustDelta::from_raw(10),
            AuditReason::try_new("Good behavior")?,
        )?;
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
