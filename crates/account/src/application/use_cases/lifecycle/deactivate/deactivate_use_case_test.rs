#[cfg(test)]
mod tests {
    use crate::application::use_cases::lifecycle::deactivate::{
        DeactivateCommand, DeactivateUseCase,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{AccountState, Email, ExternalId};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::DomainError;

    #[tokio::test]
    async fn test_deactivate_account_success() {
        let f = TestFixture::new(DeactivateUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        // 1. Arrange : Compte initial (Version 1 par défaut)
        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                region,
                Email::try_new("bye@test.com").unwrap(),
                ExternalId::from_raw("ext_123"),
            )
            .build(),
        );

        let cmd = DeactivateCommand { account_id };

        // 2. Act : On récupère l'Account
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let updated = result.unwrap();

        assert_eq!(*updated.state(), AccountState::Deactivated);
        assert_eq!(updated.version(), 2);

        let saved = f
            .identity_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert_eq!(saved.version(), 2);
        assert_eq!(
            f.outbox_repo().count(),
            1,
            "Un événement AccountEvent::DEACTIVATED attendu"
        );
        assert!(f.outbox_events().contains(&AccountEvent::DEACTIVATED.to_string()));
    }

    #[tokio::test]
    async fn test_deactivate_idempotency() {
        let f = TestFixture::new(DeactivateUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        let mut account = AccountIdentity::builder(
            account_id,
            region,
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext"),
        )
        .build();

        // On le désactive MANUELLEMENT : la version passe à 2 (si ton entité gère l'auto-incrément)
        account.deactivate().unwrap();
        f.identity_repo().insert(account);

        let cmd = DeactivateCommand { account_id };

        // 1. Act
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // 2. Assert
        assert!(result.is_ok());
        let returned = result.unwrap();

        // On vérifie que la version est restée la même que celle insérée (2)
        assert_eq!(returned.version(), 2);

        let saved = f
            .identity_repo()
            .find_by_id(&account_id)
            .expect("Should exist");

        assert_eq!(saved.version(), 2);

        // Aucun événement supplémentaire
        assert_eq!(
            f.outbox_repo().count(),
            0,
            "Idempotence : aucun événement produit"
        );
    }

    #[tokio::test]
    async fn test_deactivate_not_found() {
        let f = TestFixture::new(DeactivateUseCase::new);
        let account_id = f.account_id();

        let cmd = DeactivateCommand { account_id };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(DeactivateUseCase::new);
        let account_id = f.account_id();
        let wrong_region = RegionCode::from_raw("us");

        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                wrong_region,
                Email::try_new("hacker@test.com").unwrap(),
                ExternalId::from_raw("ext_1"),
            )
            .build(),
        );

        let cmd = DeactivateCommand { account_id };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // ASSERT : On vérifie l'obfuscation de sécurité
        // Le compte existe en base, mais le contexte doit dire "NotFound"
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}
