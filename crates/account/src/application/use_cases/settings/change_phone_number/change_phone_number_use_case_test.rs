#[cfg(test)]
mod tests {
    use crate::application::use_cases::settings::change_phone_number::{
        ChangePhoneNumberCommand, ChangePhoneNumberHandler,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{AccountState, PhoneNumber};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::{DomainError, Result};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_change_phone_number_success() -> Result<()> {
        let f = TestFixture::new();
        let old_phone = PhoneNumber::try_new("+33612345678")?;
        let new_phone = PhoneNumber::try_new("+33687654321")?;

        // 1. Arrange : Compte actif avec l'ancien téléphone
        let account = f
            .account_builder()?
            .with_state(AccountState::Active)?
            .with_phone(old_phone)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ChangePhoneNumberCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_phone: new_phone.clone(),
        };

        // 2. Act
        f.bus()
            .execute(f.account_ctx(), cmd, ChangePhoneNumberHandler)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(acc.identity().phone_number(), Some(&new_phone));
            assert!(
                !acc.identity().is_phone_verified(),
                "Le nouveau numéro ne doit pas être vérifié"
            );
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        f.assert_outbox(1, Some(AccountEvent::PHONE_NUMBER_CHANGED));

        Ok(())
    }

    #[tokio::test]
    async fn test_change_phone_technical_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let cmd_id = Uuid::new_v4();
        let requested_phone = PhoneNumber::try_new("+33611223344")?;

        // Arrange : Commande déjà connue par l'infra
        f.idempotency_repo().seed(cmd_id);

        let account = f
            .account_builder()?
            .with_state(AccountState::Active)?
            .build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ChangePhoneNumberCommand {
            command_id: cmd_id,
            account_id: f.account_id(),
            new_phone: requested_phone.clone(),
        };

        // Act
        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, ChangePhoneNumberHandler)
            .await;

        // Assert
        assert!(matches!(result, Err(DomainError::AlreadyExists { .. })));

        // VERIFICATION : L'état en base n'a pas bougé
        f.assert_account(|acc| {
            assert_ne!(acc.identity().phone_number(), Some(&requested_phone));
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        f.assert_outbox(0, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_change_phone_business_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let phone = PhoneNumber::try_new("+33600000000")?;

        // 1. Arrange : Compte possédant déjà ce numéro
        let account = f
            .account_builder()?
            .with_state(AccountState::Active)?
            .with_phone(phone.clone())
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ChangePhoneNumberCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_phone: phone.clone(),
        };

        // 2. Act
        f.bus()
            .execute(f.account_ctx(), cmd, ChangePhoneNumberHandler)
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
    async fn test_worst_case_outbox_failure_propagation() -> Result<()> {
        let f = TestFixture::new();
        let error_msg = "Kafka/Outbox DB Error";

        let account = f
            .account_builder()?
            .with_state(AccountState::Active)?
            .build()?;

        f.account_repo().insert(account);

        // Simulation d'une erreur d'infrastructure lors du commit (Outbox)
        f.outbox_repo()
            .set_error(DomainError::Infrastructure(error_msg.into()));

        let requested_phone = PhoneNumber::try_new("+33611223344")?;
        let cmd = ChangePhoneNumberCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_phone: requested_phone.clone(),
        };

        // 2. Act
        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, ChangePhoneNumberHandler)
            .await;

        // 3. Assert
        // On vérifie que l'erreur d'infrastructure est bien remontée au Bus
        assert!(matches!(result, Err(DomainError::Infrastructure(m)) if m == error_msg));

        // NOTE : On ne vérifie pas le rollback ici.
        // Pourquoi ? Parce que f.account_repo() est un InMemoryRepository.
        // Sans une vraie base de données (Postgres), l'annulation atomique
        // des changements en mémoire n'est pas possible via FakeTransaction.
        // Ce test de "vrai rollback" appartient aux tests d'intégration (IT).

        Ok(())
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() -> Result<()> {
        let f = TestFixture::new();
        let wrong_region = RegionCode::from_raw("us");

        // Compte US dans un contexte européen
        let account = f
            .account_builder_for(wrong_region)?
            .with_state(AccountState::Active)?
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ChangePhoneNumberCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_phone: PhoneNumber::try_new("+33611223344")?,
        };

        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, ChangePhoneNumberHandler)
            .await;

        assert!(matches!(result, Err(DomainError::NotFound { .. })));

        // Vérification directe via le repo (car f.assert_account échouerait sur le RegionCheck)
        let saved = f.account_repo().find_direct(&f.account_id()).unwrap();
        assert_eq!(saved.version(), version_snapshot);

        Ok(())
    }
}
