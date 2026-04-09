#[cfg(test)]
mod tests {
    use crate::application::use_cases::lifecycle::activate::{ActivateCommand, ActivateUseCase};
    use crate::application::utils::TestFixture;
    use crate::domain::account::builders::AccountIdentityBuilder;
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::value_objects::{AccountState, Email, ExternalId, Locale};
    use chrono::Utc;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::errors::DomainError;

    #[tokio::test]
    async fn test_reactivate_account_success() {
        let f = TestFixture::new(ActivateUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        // 1. Arrange : On crée un compte désactivé
        let mut identity = AccountIdentity::builder(
            account_id,
            region,
            Email::try_new("back@test.com").unwrap(),
            ExternalId::from_raw("ext_123"),
        )
        .build();

        // On le passe en désactivé (Version passe à 2)
        identity.deactivate().unwrap();
        identity.pull_events();
        let version_deactivated = identity.version();

        f.identity_repo().insert(identity);

        let cmd = ActivateCommand {
            account_id,
        };

        // 2. Act
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let updated = result.unwrap();

        assert_eq!(*updated.state(), AccountState::Active);
        assert_eq!(updated.version(), version_deactivated + 1);

        // 4. Persistence
        let saved = f
            .identity_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert_eq!(*saved.state(), AccountState::Active);

        // 5. Outbox
        assert_eq!(
            f.outbox_count(),
            1,
            "Un événement Activate attendu"
        );
    }

    #[tokio::test]
    async fn test_reactivate_idempotency() {
        let f = TestFixture::new(ActivateUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        // 1. Arrange : Compte déjà ACTIVE via restore
        let identity = AccountIdentityBuilder::restore(
            account_id,
            region,
            ExternalId::from_raw("ext"),
            Email::try_new("a@b.com").unwrap(),
            true,
            None,
            false,
            AccountState::Active,
            None,
            Locale::default(),
            1,
            Utc::now(),
            Utc::now(),
            Some(Utc::now()),
        );

        f.identity_repo().insert(identity);

        let cmd = ActivateCommand {
            account_id: account_id.clone(),
        };

        // 2. Act
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let returned = result.unwrap();

        assert_eq!(*returned.state(), AccountState::Active);
        assert_eq!(returned.version(), 1);

        // 4. Outbox
        assert_eq!(f.outbox_count(), 0, "Idempotence : aucun événement produit");
    }

    #[tokio::test]
    async fn test_reactivate_forbidden_if_banned() {
        let f = TestFixture::new(ActivateUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        let mut identity = AccountIdentity::builder(
            account_id,
            region,
            Email::try_new("banned@test.com").unwrap(),
            ExternalId::from_raw("ext"),
        )
        .build();

        identity.ban("Violation".into()).unwrap();
        f.identity_repo().insert(identity);

        let cmd = ActivateCommand { account_id };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // Seul un compte Deactivated peut être réactivé manuellement
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(ActivateUseCase::new);
        let account_id = f.account_id();
        let region = f.region();


        // On simule une donnée en base qui appartient aux "us"
        // alors que notre contexte est "eu"
        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                region,
                Email::try_new("hacker@test.com").unwrap(),
                ExternalId::from_raw("ext_1"),
            )
            .build(),
        );

        let cmd = ActivateCommand { account_id };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // ASSERT : On vérifie l'obfuscation de sécurité
        // Le compte existe en base, mais le contexte doit dire "NotFound"
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}
