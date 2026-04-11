#[cfg(test)]
mod tests {
    use crate::application::use_cases::moderation::ban::{BanCommand, BanUseCase};
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{AccountState, Email, ExternalId};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::DomainError;

    #[tokio::test]
    async fn test_ban_account_success() {
        let f = TestFixture::new(BanUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        // Création d'un compte actif
        let identity = AccountIdentity::builder(
            account_id,
            region,
            Email::try_new("user@example.com").unwrap(),
            ExternalId::from_raw("ext_123"),
        )
        .build();
        f.identity_repo().insert(identity);

        let cmd = BanCommand {
            account_id,
            reason: "TOS Violation".into(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        assert!(result.is_ok());
       let saved = f
            .identity_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert_eq!(*saved.state(), AccountState::Banned);
        assert_eq!(
            f.outbox_repo().count(),
            1,
            "Un événement AccountEvent::BANNED attendu"
        );
        assert!(f.outbox_events().contains(&AccountEvent::BANNED.to_string()));
    }


    #[tokio::test]
    async fn test_ban_account_idempotency_no_double_ban() {
        let f = TestFixture::new(BanUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        let mut identity = AccountIdentity::builder(
            account_id,
            region,
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext"),
        )
        .build();

        identity.ban("First reason".into()).unwrap();
        f.identity_repo().insert(identity);

        let cmd = BanCommand {
            account_id,
            reason: "Second attempt".into(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd.clone()).await;

        assert!(result.is_ok());
       let saved = f
            .identity_repo()
            .find_by_id(&account_id)
            .expect("Should exist");

        // La version ne doit pas avoir bougé (idempotence)
        assert_eq!(saved.version(), 2);
        assert_eq!(
            f.outbox_repo().count(),
            0,
            "Idempotence : aucun événement généré"
        );
    }

    #[tokio::test]
    async fn test_ban_account_full_success_flow() {
        let f = TestFixture::new(BanUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        // Arrange: Compte existant
        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                region,
                Email::try_new("troll@internet.com").unwrap(),
                ExternalId::from_raw("ext_1"),
            )
            .build(),
        );

        let cmd = BanCommand {
            account_id,
            reason: "Spamming".into(),
        };

        // Act
        let result = f.use_case().execute(f.ctx(), cmd).await;

        // Assert
        assert!(result.is_ok());
        let saved = f
            .identity_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert_eq!(*saved.state(), AccountState::Banned);
        assert_eq!(
            f.outbox_repo().count(),
            1,
            "Un événement Ban attendu"
        );
    }

    #[tokio::test]
    async fn test_worst_case_account_not_found() {
        let f = TestFixture::new(BanUseCase::new);
        let account_id = f.account_id();

        let cmd = BanCommand {
            account_id,
            reason: "No matter".into(),
        };

        let result = f.use_case().execute(f.ctx(), cmd).await;

        // On vérifie que l'erreur NotFound remonte correctement
        assert!(matches!(
            result,
            Err(DomainError::NotFound {
                entity: "AccountIdentity",
                ..
            })
        ));
    }

    #[tokio::test]
    async fn test_worst_case_concurrency_exhaustion() {
        let f = TestFixture::new(BanUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                region,
                Email::try_new("a@b.com").unwrap(),
                ExternalId::from_raw("ext"),
            )
            .build(),
        );

        // Simulation d'un conflit de version PERMANENT (ex: DB lock)
        f.identity_repo().set_error(DomainError::ConcurrencyConflict {
            reason: "Always failing".into(),
        });

        let cmd = BanCommand {
            account_id,
            reason: "Ban".into(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // Le retry doit finir par abandonner et rendre l'erreur
        assert!(matches!(
            result,
            Err(DomainError::ConcurrencyConflict { .. })
        ));
    }

    #[tokio::test]
    async fn test_worst_case_atomic_outbox_failure_propagation() {
        let f = TestFixture::new(BanUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                region,
                Email::try_new("a@b.com").unwrap(),
                ExternalId::from_raw("ext"),
            )
            .build(),
        );

        // On simule une erreur lors de l'écriture de l'outbox (transaction fail)
        f.outbox_repo().set_error(DomainError::Internal("Disk full".into()));

        let cmd = BanCommand {
            account_id,
            reason: "Ban".into(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // Le Use Case doit échouer et l'erreur de l'outbox doit remonter
        assert!(matches!(result, Err(DomainError::Internal(m)) if m == "Disk full"));
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(BanUseCase::new);
        let account_id = f.account_id();
        let wrong_region = RegionCode::from_raw("us");

        // On simule une donnée en base qui appartient aux "us"
        // alors que notre contexte est "eu"
        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                wrong_region,
                Email::try_new("hacker@test.com").unwrap(),
                ExternalId::from_raw("ext_1"),
            )
            .build(),
        );
        let cmd = BanCommand {
            account_id,
            reason: "Ban".into(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}
