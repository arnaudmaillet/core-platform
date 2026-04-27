#[cfg(test)]
mod tests {
    use crate::application::use_cases::settings::change_birth_date::{
        ChangeBirthDateCommand, ChangeBirthDateHandler,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{AccountState, BirthDate};
    use chrono::NaiveDate;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::{DomainError, Result};
    use uuid::Uuid;

    fn adult_birth_date() -> BirthDate {
        BirthDate::try_new(NaiveDate::from_ymd_opt(2000, 1, 1).unwrap()).unwrap()
    }

    #[tokio::test]
    async fn test_change_birth_date_success() -> Result<()> {
        let f = TestFixture::new();

        let account = f
            .account_builder()?
            .with_state(AccountState::Active)?
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let new_date = adult_birth_date();
        let cmd = ChangeBirthDateCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_birth_date: new_date.clone(),
        };

        f.bus()
            .execute(f.account_ctx(), cmd, ChangeBirthDateHandler)
            .await?;

        f.assert_account(|acc| {
            assert_eq!(acc.identity().birth_date(), Some(&new_date));
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        f.assert_outbox(1, Some(AccountEvent::BIRTH_DATE_CHANGED));

        Ok(())
    }

    #[tokio::test]
    async fn test_change_birth_date_technical_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let cmd_id = Uuid::new_v4();

        f.idempotency_repo().seed(cmd_id);

        let account = f
            .account_builder()?
            .with_state(AccountState::Active)?
            .build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let new_date = adult_birth_date();
        let cmd = ChangeBirthDateCommand {
            command_id: cmd_id,
            account_id: f.account_id(),
            new_birth_date: new_date.clone(),
        };

        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, ChangeBirthDateHandler)
            .await;

        assert!(matches!(result, Err(DomainError::AlreadyExists { .. })));

        // Assert : Rien n'a bougé
        f.assert_account(|acc| {
            assert_ne!(acc.identity().birth_date(), Some(&new_date));
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        f.assert_outbox(0, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_change_birth_date_forbidden_when_restricted() -> Result<()> {
        let f = TestFixture::new();

        let account = f
            .account_builder()?
            .with_state(AccountState::Banned)?
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let new_date = adult_birth_date();
        let cmd = ChangeBirthDateCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_birth_date: new_date.clone(),
        };

        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, ChangeBirthDateHandler)
            .await;

        assert!(matches!(result, Err(DomainError::Forbidden { .. })));

        // Assert : Intégrité conservée
        f.assert_account(|acc| {
            assert_ne!(acc.identity().birth_date(), Some(&new_date));
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_worst_case_concurrency_conflict() -> Result<()> {
        let f = TestFixture::new();
        let account = f
            .account_builder()?
            .with_state(AccountState::Active)?
            .build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        f.account_repo()
            .set_error(DomainError::ConcurrencyConflict {
                reason: "Optimistic lock failure".into(),
            });

        let new_date = adult_birth_date();
        let cmd = ChangeBirthDateCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_birth_date: new_date.clone(),
        };

        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, ChangeBirthDateHandler)
            .await;
        assert!(matches!(
            result,
            Err(DomainError::ConcurrencyConflict { .. })
        ));

        // Assert : Rollback logique
        f.assert_account(|acc| {
            assert_ne!(acc.identity().birth_date(), Some(&new_date));
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() -> Result<()> {
        let f = TestFixture::new();
        let wrong_region = RegionCode::from_raw("us");

        let account = f
            .account_builder_for(wrong_region)?
            .with_state(AccountState::Active)?
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ChangeBirthDateCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_birth_date: adult_birth_date(),
        };

        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, ChangeBirthDateHandler)
            .await;

        assert!(matches!(result, Err(DomainError::NotFound { .. })));

        // On vérifie directement via le repo
        let saved = f.account_repo().find_direct(&f.account_id()).unwrap();
        assert_eq!(saved.version(), version_snapshot);

        Ok(())
    }
}
