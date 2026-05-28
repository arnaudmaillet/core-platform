#[cfg(test)]
mod tests {
    use crate::application::commands::settings::ChangeBirthDateCommand;
    use crate::application::context::AccountCommandContext;
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::types::{AccountState, BirthDate};
    use chrono::NaiveDate;
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::{Error, ErrorCode, Result, Versioned};
    use uuid::Uuid;

    fn adult_birth_date() -> BirthDate {
        BirthDate::try_new(NaiveDate::from_ymd_opt(2000, 1, 1).unwrap()).unwrap()
    }

    #[tokio::test]
    async fn test_change_birth_date_success() -> Result<()> {
        let f = TestFixture::new();

        let account = f
            .builder()?
            .with_state(AccountState::ACTIVE)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let new_date = adult_birth_date();
        let cmd = ChangeBirthDateCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            new_birth_date: new_date.clone(),
        };

        f.bus()
            .execute::<AccountCommandContext, ChangeBirthDateCommand, ()>(f.command_ctx().clone(), cmd)
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
            .builder()?
            .with_state(AccountState::ACTIVE)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let new_date = adult_birth_date();
        let cmd = ChangeBirthDateCommand {
            command_id: cmd_id,
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            new_birth_date: new_date.clone(),
        };

        let result = f
            .bus()
            .execute::<AccountCommandContext, ChangeBirthDateCommand, ()>(f.command_ctx().clone(), cmd)
            .await;

        assert!(
            result.is_ok(),
            "L'idempotence technique doit être transparente (Ok)"
        );

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
            .builder()?
            .with_state(AccountState::BANNED)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let new_date = adult_birth_date();
        let cmd = ChangeBirthDateCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            new_birth_date: new_date.clone(),
        };

        let result = f
            .bus()
            .execute::<AccountCommandContext, ChangeBirthDateCommand, ()>(f.command_ctx().clone(), cmd)
            .await;

        match result {
            Err(e) => {
                assert_eq!(e.code, ErrorCode::Forbidden);
            }
            Ok(_) => panic!("Should have failed: a banned account cannot change its state"),
        }

        // Assert : Intégrité conservée
        f.assert_account(|acc| {
            assert_ne!(acc.identity().birth_date(), Some(&new_date));
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_change_birth_date_succeeds_after_retry() -> Result<()> {
        let f = TestFixture::new();
        let account = f
            .builder()?
            .with_state(AccountState::ACTIVE)
            .build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        // 1. Arrange : On simule un conflit de version SQL (OCC)
        // Le stub renverra l'erreur une seule fois (take() interne)
        f.account_repo()
            .set_error_once(Error::concurrency_conflict("Optimistic lock failure"));

        let new_date = adult_birth_date();
        let cmd = ChangeBirthDateCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            new_birth_date: new_date.clone(),
        };

        // 2. Act : Le Bus intercepte le ConcurrencyConflict et relance le Handler
        let result = f
            .bus()
            .execute::<AccountCommandContext, ChangeBirthDateCommand, ()>(f.command_ctx().clone(), cmd)
            .await;

        // 3. Assert : Succès final attendu
        assert!(result.is_ok(), "Le bus aurait dû retenter et réussir");

        f.assert_account(|acc| {
            // La donnée doit être à jour car le retry a réussi
            assert_eq!(acc.identity().birth_date(), Some(&new_date));
            // La version a augmenté de 1
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        // L'événement doit être dans l'outbox
        f.assert_outbox(1, Some(AccountEvent::BIRTH_DATE_CHANGED));

        Ok(())
    }
}
