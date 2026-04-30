#[cfg(test)]
mod tests {
    use crate::application::use_cases::access_management::verify_phone_number::{
        VerifyPhoneNumberCommand, VerifyPhoneNumberHandler,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::VerificationToken;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::{DomainError, Result};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_verify_phone_success() -> Result<()> {
        let f = TestFixture::new();
        // Utilisation du builder de la fixture
        let account = f.account_builder()?.build()?;
        let version_snapshot = account.version();

        f.account_repo().insert(account);

        // Le token est la preuve fournie par le webhook Keycloak
        let token = VerificationToken::try_new("keycloak_sms_verified_proof_123")?;

        let cmd = VerifyPhoneNumberCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            token,
        };

        // 2. Act
        f.bus()
            .execute(f.account_ctx(), cmd, VerifyPhoneNumberHandler)
            .await?;

        // 3. Assert via DSL
        f.assert_account(|acc| {
            assert!(acc.identity().is_phone_verified());
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        f.assert_outbox(1, Some(AccountEvent::PHONE_VERIFIED));
        Ok(())
    }

    #[tokio::test]
    async fn test_verify_phone_business_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let token = VerificationToken::try_new("proof_already_processed")?;

        // 1. Arrange : On simule un téléphone déjà vérifié dans l'agrégat
        let mut account = f.account_builder()?.build()?;
        account.verify_phone(token.clone())?;
        account.pull_events(); // Vider l'outbox initiale
        let version_snapshot = account.version();

        f.account_repo().insert(account);

        let cmd = VerifyPhoneNumberCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            token,
        };

        // 2. Act
        f.bus()
            .execute(f.account_ctx(), cmd, VerifyPhoneNumberHandler)
            .await?;

        // 3. Assert : Rien ne doit changer (Idempotence)
        f.assert_account(|acc| {
            assert!(acc.identity().is_phone_verified());
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        f.assert_outbox(0, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_verify_phone_technical_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let cmd_id = Uuid::new_v4();

        // Simule que l'infrastructure a déjà vu cet ID de commande
        f.idempotency_repo().seed(cmd_id);

        let account = f.account_builder()?.build()?;
        f.account_repo().insert(account);

        let cmd = VerifyPhoneNumberCommand {
            command_id: cmd_id,
            account_id: f.account_id(),
            token: VerificationToken::try_new("any_token")?,
        };

        // 2. Act
        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, VerifyPhoneNumberHandler)
            .await;

        // 3. Assert : Rejet technique avant le domaine
        assert!(matches!(result, Err(DomainError::AlreadyExists { .. })));
        f.assert_outbox(0, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() -> Result<()> {
        let f = TestFixture::new();
        let wrong_region = RegionCode::from_raw("us");

        // Arrange : Donnée aux US, contexte en EU
        let account = f.account_builder_for(wrong_region)?.build()?;
        f.account_repo().insert(account);

        let cmd = VerifyPhoneNumberCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            token: VerificationToken::try_new("some_token")?,
        };

        // 2. Act
        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, VerifyPhoneNumberHandler)
            .await;

        // 3. Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
        Ok(())
    }
}
