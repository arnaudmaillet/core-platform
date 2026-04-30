#[cfg(test)]
mod tests {
    use crate::application::use_cases::settings::change_email::{
        ChangeEmailCommand, ChangeEmailHandler,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{AccountState, Email};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::{DomainError, Result};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_change_email_success() -> Result<()> {
        let f = TestFixture::new();
        let old_email = Email::try_new("old@test.com")?;
        let new_email = Email::try_new("new@test.com")?;

        // 1. Arrange : Compte actif avec l'ancien email
        let account = f
            .account_builder()?
            .with_state(AccountState::Active)
            .with_email(old_email)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ChangeEmailCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_email: new_email.clone(),
        };

        // 2. Act
        f.bus()
            .execute(f.account_ctx(), cmd, ChangeEmailHandler)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(acc.identity().email(), Some(&new_email));
            assert!(
                !acc.identity().is_email_verified(),
                "L'email ne doit plus être vérifié"
            );
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        f.assert_outbox(1, Some(AccountEvent::EMAIL_CHANGED));

        Ok(())
    }

    #[tokio::test]
    async fn test_change_email_technical_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let cmd_id = Uuid::new_v4();
        let requested_email = Email::try_new("other@test.com")?;

        // Arrange : Commande déjà enregistrée
        f.idempotency_repo().seed(cmd_id);

        let account = f
            .account_builder()?
            .with_state(AccountState::Active)
            .build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ChangeEmailCommand {
            command_id: cmd_id,
            account_id: f.account_id(),
            new_email: requested_email.clone(),
        };

        // Act
        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, ChangeEmailHandler)
            .await;

        // Assert
        assert!(matches!(result, Err(DomainError::AlreadyExists { .. })));

        // VERIFICATION : L'email n'a pas été modifié
        f.assert_account(|acc| {
            assert_ne!(acc.identity().email(), Some(&requested_email));
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        f.assert_outbox(0, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_change_email_business_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let email = Email::try_new("same@test.com")?;

        // 1. Arrange : Compte possédant déjà cet email
        let account = f
            .account_builder()?
            .with_state(AccountState::Active)
            .with_email(email.clone())
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ChangeEmailCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_email: email,
        };

        // 2. Act
        f.bus()
            .execute(f.account_ctx(), cmd, ChangeEmailHandler)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(
                acc.version(),
                version_snapshot,
                "La version ne doit pas bouger"
            );
        })
        .await?;

        f.assert_outbox(0, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_change_email_forbidden_when_restricted() -> Result<()> {
        let f = TestFixture::new();
        let requested_email = Email::try_new("new@test.com")?;

        // Arrange : Un banni ne peut pas modifier ses réglages
        let account = f
            .account_builder()?
            .with_state(AccountState::Banned)
            .build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ChangeEmailCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_email: requested_email.clone(),
        };

        // Act
        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, ChangeEmailHandler)
            .await;

        // Assert
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));

        // VERIFICATION : Intégrité conservée
        f.assert_account(|acc| {
            assert_ne!(acc.identity().email(), Some(&requested_email));
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_change_email_succeeds_after_retry() -> Result<()> {
        let f = TestFixture::new();
        let requested_email = Email::try_new("b@c.com")?;

        let account = f
            .account_builder()?
            .with_state(AccountState::Active)
            .build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        // 1. Arrange : Simulation d'une erreur OCC (Optimistic Concurrency Control)
        // Le repo renverra l'erreur une seule fois, puis réussira au retry.
        f.account_repo()
            .set_error_once(DomainError::ConcurrencyConflict {
                reason: "Version mismatch".into(),
            });

        let cmd = ChangeEmailCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_email: requested_email.clone(),
        };

        // 2. Act : Le Bus intercepte le conflit et relance le handler
        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, ChangeEmailHandler)
            .await;

        // 3. Assert : On s'attend maintenant à un SUCCÈS
        assert!(
            result.is_ok(),
            "Le retry automatique aurait dû sauver l'opération"
        );

        f.assert_account(|acc| {
            // L'email a bien été mis à jour après le retry
            assert_eq!(acc.identity().email(), Some(&requested_email));
            // La version a bien été incrémentée
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        // L'événement doit être présent dans l'outbox
        f.assert_outbox(1, Some(AccountEvent::EMAIL_CHANGED));

        Ok(())
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() -> Result<()> {
        let f = TestFixture::new();
        let wrong_region = RegionCode::from_raw("us");

        let account = f
            .account_builder_for(wrong_region)?
            .with_state(AccountState::Active)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ChangeEmailCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_email: Email::try_new("new@test.com")?,
        };

        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, ChangeEmailHandler)
            .await;

        assert!(matches!(result, Err(DomainError::NotFound { .. })));

        // Vérification directe (le contexte ne voit pas le compte)
        let saved = f.account_repo().find_direct(&f.account_id()).unwrap();
        assert_eq!(saved.version(), version_snapshot);
        f.assert_outbox(0, None);

        Ok(())
    }
}
