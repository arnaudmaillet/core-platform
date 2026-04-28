#[cfg(test)]
mod tests {
    use crate::application::use_cases::access_management::verify_email::{
        VerifyEmailCommand, VerifyEmailHandler,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{Email, VerificationToken};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::{DomainError, Result};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_verify_email_success() -> Result<()> {
        let f = TestFixture::new();
        let account = f
            .account_builder()?
            .with_email(Email::try_new("user@test.com")?)
            .build()?;

        let version_snapshot = account.version();
        let token = VerificationToken::try_new("keycloak_webhook_signature")?;

        let cmd = VerifyEmailCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            token,
        };

        f.account_repo().insert(account);

        let _ = f
            .bus()
            .execute(f.account_ctx(), cmd, VerifyEmailHandler)
            .await;

        f.assert_account(|acc| {
            assert!(acc.identity().is_email_verified());
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        f.assert_outbox(1, Some(AccountEvent::EMAIL_VERIFIED));
        Ok(())
    }

    #[tokio::test]
    async fn test_verify_email_technical_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let cmd_id = Uuid::new_v4();
        let token = VerificationToken::try_new("duplicate_webhook_call")?;

        f.idempotency_repo().seed(cmd_id);

        let account = f.account_builder()?.build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = VerifyEmailCommand {
            command_id: cmd_id,
            account_id: f.account_id(),
            token,
        };

        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, VerifyEmailHandler)
            .await;

        assert!(matches!(result, Err(DomainError::AlreadyExists { .. })));

        f.assert_account(|acc| {
            assert!(!acc.identity().is_email_verified());
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        f.assert_outbox(0, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_verify_email_business_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let token = VerificationToken::try_new("initial_token")?;

        let mut account = f.account_builder()?.build()?;
        // On passe la référence du VO à l'agrégat
        account.verify_email(token)?;
        account.pull_events();
        let version_snapshot = account.version();

        f.account_repo().insert(account);

        let cmd = VerifyEmailCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            token: VerificationToken::try_new("new_request_for_same_state")?,
        };

        f.bus()
            .execute(f.account_ctx(), cmd, VerifyEmailHandler)
            .await?;

        f.assert_account(|acc| {
            assert!(acc.identity().is_email_verified());
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        f.assert_outbox(0, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_verify_email_concurrency_retry() -> Result<()> {
        let f = TestFixture::new();
        let account = f.account_builder()?.build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        // On s'assure que le slot est VIDE avant de commencer
        // (Au cas où insert() aurait laissé une traînée)

        f.account_repo()
            .set_error_once(DomainError::ConcurrencyConflict {
                reason: "Race condition".into(),
            });

        let cmd = VerifyEmailCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            token: VerificationToken::try_new("sig_retry")?,
        };

        // Si ça échoue ici, c'est que le bus a fait ses 3 tentatives
        // et qu'elles ont TOUTES renvoyé une erreur.
        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, VerifyEmailHandler)
            .await;

        if let Err(ref e) = result {
            println!("DEBUG: Final error received: {:?}", e);
        }

        result?;

        f.assert_account(|acc| {
            assert!(acc.identity().is_email_verified());
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        f.assert_outbox(1, Some(AccountEvent::EMAIL_VERIFIED));
        Ok(())
    }

    #[tokio::test]
    async fn test_verify_email_region_mismatch_returns_not_found() -> Result<()> {
        let f = TestFixture::new();
        let wrong_region = RegionCode::from_raw("us");

        let account = f.account_builder_for(wrong_region)?.build()?;
        let version_snapshot = account.version();

        f.account_repo().insert(account);

        let cmd = VerifyEmailCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            token: VerificationToken::try_new("keycloak_sig_from_wrong_region")?,
        };

        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, VerifyEmailHandler)
            .await;

        assert!(matches!(result, Err(DomainError::NotFound { .. })));

        let saved = f.account_repo().find_direct(&f.account_id()).unwrap();
        assert_eq!(saved.version(), version_snapshot);
        f.assert_outbox(0, None);

        Ok(())
    }
}
