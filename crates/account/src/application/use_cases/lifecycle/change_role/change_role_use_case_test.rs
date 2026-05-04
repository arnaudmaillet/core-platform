#[cfg(test)]
mod tests {
    use crate::application::context::AccountContext;
    use crate::application::use_cases::lifecycle::{ChangeRoleCommand, ChangeRoleHandler};
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::AccountRole;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::{AuditReason, RegionCode};
    use shared_kernel::errors::{DomainError, Result};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_change_role_success() -> Result<()> {
        let f = TestFixture::new();
        let account = f.account_builder()?.build()?;
        let version_snapshot = account.version();

        f.account_repo().insert(account);

        let cmd = ChangeRoleCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_role: AccountRole::MODERATOR,
            reason: AuditReason::try_new("Joined the safety team")?,
        };

        f.bus()
            .execute::<AccountContext, ChangeRoleCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        f.assert_account(|acc| {
            assert_eq!(acc.governance().role(), AccountRole::MODERATOR);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        f.assert_outbox(1, Some(AccountEvent::ROLE_CHANGED));

        Ok(())
    }

    #[tokio::test]
    async fn test_change_role_technical_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let cmd_id = Uuid::new_v4();

        f.idempotency_repo().seed(cmd_id);

        let mut account = f.account_builder()?.build()?;
        let _ = account.change_role(AccountRole::MODERATOR, AuditReason::try_new("init")?);
        account.pull_events();

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ChangeRoleCommand {
            command_id: cmd_id,
            account_id: f.account_id(),
            new_role: AccountRole::MODERATOR,
            reason: AuditReason::try_new("Duplicate promotion")?,
        };

        let result = f
            .bus()
            .execute::<AccountContext, ChangeRoleCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        assert!(matches!(result, Err(DomainError::AlreadyExists { .. })));

        f.assert_account(|acc| {
            assert_eq!(acc.governance().role(), AccountRole::MODERATOR);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_change_role_business_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let mut account = f.account_builder()?.build()?;

        let _ = account.change_role(AccountRole::MODERATOR, AuditReason::try_new("init")?);
        account.pull_events();

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ChangeRoleCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_role: AccountRole::MODERATOR,
            reason: AuditReason::try_new("Duplicate promotion")?,
        };

        f.bus()
            .execute::<AccountContext, ChangeRoleCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        f.assert_account(|acc| {
            assert_eq!(acc.governance().role(), AccountRole::MODERATOR);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        f.assert_outbox(0, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() -> Result<()> {
        let f = TestFixture::new();
        let wrong_region = RegionCode::from_raw("US");

        let account = f.account_builder_for(wrong_region)?.build()?;
        let version_snapshot = account.version();

        f.account_repo().insert(account);

        let cmd = ChangeRoleCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_role: AccountRole::MODERATOR,
            reason: AuditReason::try_new("some_reason")?,
        };

        let result = f
            .bus()
            .execute::<AccountContext, ChangeRoleCommand, ()>(f.account_ctx().clone(), cmd)
            .await;
        assert!(matches!(result, Err(DomainError::NotFound { .. })));

        // Vérification directe via le repo (car le contexte ne le trouvera pas)
        let saved = f.account_repo().find_direct(&f.account_id()).unwrap();
        assert_eq!(saved.version(), version_snapshot);
        f.assert_outbox(0, None);
        Ok(())
    }
}
