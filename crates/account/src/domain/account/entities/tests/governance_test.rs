#[cfg(test)]
mod tests {
    use chrono::Utc;
    use shared_kernel::{
        domain::{
            entities::Entity,
            value_objects::{AccountId, AuditReason, TrustContext},
        },
        errors::Result,
    };

    use crate::domain::{
        account::entities::AccountGovernance,
        value_objects::{AccountRole, IpAddr, TrustDelta, TrustScore},
    };

    fn create_test_governance() -> Result<AccountGovernance> {
        let account_id = AccountId::new();
        let ip_addr = IpAddr::try_new("127.0.0.1")?;

        // Utilisation du restore simplifié (sans metadata/version/updated_at)
        Ok(AccountGovernance::restore(
            account_id,
            AccountRole::USER,
            false,
            false,
            TrustScore::new_max(),
            None,
            None,
            Some(ip_addr),
            Utc::now(),
        ))
    }

    #[test]
    fn test_initial_state_and_getters() -> Result<()> {
        let gov = create_test_governance()?;
        let expected_ip = IpAddr::try_new("127.0.0.1")?;

        assert_eq!(gov.role(), AccountRole::USER);
        assert_eq!(gov.trust_score().value(), 100);
        assert_eq!(gov.last_ip_addr(), Some(&expected_ip));

        assert!(gov.updated_at() <= Utc::now());

        Ok(())
    }

    #[test]
    fn test_trust_reward_and_clamping() -> Result<()> {
        let mut gov = create_test_governance()?;
        let mut reason = AuditReason::try_new("Good behavior")?;

        // Déjà à 100, une récompense ne doit rien changer (idempotence)
        let changed = gov.apply_trust_reward(
            TrustDelta::from_raw(10),
            TrustContext::ManualAdjustment,
            &reason,
        )?;
        assert!(!changed);
        assert_eq!(gov.trust_score().value(), 100);

        reason = AuditReason::try_new("Penalty")?;

        // On baisse pour tester la remontée
        gov.apply_trust_penalty(
            TrustDelta::from_raw(20),
            TrustContext::ManualAdjustment,
            &reason,
        )?;
        assert_eq!(gov.trust_score().value(), 80);

        reason = AuditReason::try_new("Bouncing back")?;

        let changed = gov.apply_trust_reward(
            TrustDelta::from_raw(10),
            TrustContext::ManualAdjustment,
            &reason,
        )?;
        assert!(changed);
        assert_eq!(gov.trust_score().value(), 90);

        Ok(())
    }

    #[test]
    fn test_shadowban_lifecycle_idempotency() -> Result<()> {
        let mut gov = create_test_governance()?;

        let mut reason = AuditReason::try_new("Investigation")?;

        // Shadowban manuel
        let changed = gov.apply_shadowban(&reason).unwrap();
        assert!(changed);

        // Idempotence
        reason = AuditReason::try_new("Same reason")?;
        let changed_again = gov.apply_shadowban(&reason).unwrap();
        assert!(!changed_again);

        // Levée du shadowban
        reason = AuditReason::try_new("Cleared")?;
        let changed = gov.apply_lift_shadowban(&reason).unwrap();
        assert!(changed);
        assert!(!gov.is_shadowbanned());

        Ok(())
    }

    #[test]
    fn test_role_change_and_logging() -> Result<()> {
        let mut gov = create_test_governance()?;
        let mut reason = AuditReason::try_new("Promotion")?;

        let changed = gov.apply_role_change(AccountRole::STAFF, &reason).unwrap();
        assert!(changed);
        assert_eq!(gov.role(), AccountRole::STAFF);
        // Vérification que le log a été écrit
        assert!(gov.moderation_notes().unwrap().contains("Role changed"));

        assert!(gov.moderation_notes().unwrap().contains("staff"));

        // Idempotence
        reason = AuditReason::try_new("Duplicate")?;
        let changed = gov.apply_role_change(AccountRole::STAFF, &reason).unwrap();
        assert!(!changed);

        Ok(())
    }

    #[test]
    fn test_beta_status_toggle() -> Result<()> {
        let mut gov = create_test_governance()?;
        let mut reason = AuditReason::try_new("Feature testing")?;

        let changed = gov.apply_beta_status(true, &reason).unwrap();
        assert!(changed);
        assert!(gov.is_beta_tester());

        reason = AuditReason::try_new("Already in")?;
        let changed = gov.apply_beta_status(true, &reason).unwrap();
        assert!(!changed);

        Ok(())
    }

    #[test]
    fn test_ip_record_update() -> Result<()> {
        let mut gov = create_test_governance()?;
        let new_ip = IpAddr::try_new("192.168.1.1")?;

        gov.apply_ip_record(new_ip.clone());
        assert_eq!(gov.last_ip_addr(), Some(&new_ip));

        Ok(())
    }
}
