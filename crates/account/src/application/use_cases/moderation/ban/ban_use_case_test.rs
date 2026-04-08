#[cfg(test)]
mod tests {
    use crate::application::use_cases::moderation::ban::{BanCommand, BanUseCase};
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::repositories::AccountIdentityRepositoryStub;
    use crate::domain::value_objects::{AccountState, Email, ExternalId};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;
    use std::sync::Arc;

    fn setup() -> (
        BanUseCase,
        Arc<AccountIdentityRepositoryStub>,
        Arc<OutboxRepositoryStub>,
    ) {
        let account_repo = Arc::new(AccountIdentityRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = BanUseCase::new(account_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, account_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_ban_account_success() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // Création d'un compte actif
        let account = AccountIdentity::builder(
            account_id.clone(),
            region.clone(),
            Email::try_new("user@example.com").unwrap(),
            ExternalId::from_raw("ext_123"),
        )
        .build();
        account_repo.insert(account);

        let cmd = BanCommand {
            account_id: account_id.clone(),
            reason: "TOS Violation".into(),
        };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        let saved = account_repo
            .identity_map
            .lock()
            .unwrap()
            .get(&account_id)
            .cloned()
            .unwrap();
        assert_eq!(*saved.state(), AccountState::Banned);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }


    #[tokio::test]
    async fn test_ban_account_idempotency_no_double_ban() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        let mut account = AccountIdentity::builder(
            account_id.clone(),
            region.clone(),
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext"),
        )
        .build();

        account.ban("First reason".into()).unwrap();
        account_repo.add_account(account);

        let cmd = BanCommand {
            account_id,
            reason: "Second attempt".into(),
        };

        let result = use_case.execute(cmd.clone()).await;

        assert!(result.is_ok());
        let saved = account_repo
            .identity_map
            .lock()
            .unwrap()
            .get(&cmd.account_id)
            .unwrap()
            .clone();

        // La version ne doit pas avoir bougé (idempotence)
        assert_eq!(saved.version(), 2);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_ban_account_full_success_flow() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // Arrange: Compte existant
        account_repo.insert(
            AccountIdentity::builder(
                account_id.clone(),
                region.clone(),
                Email::try_new("troll@internet.com").unwrap(),
                ExternalId::from_raw("ext_1"),
            )
            .build(),
        );

        let cmd = BanCommand {
            account_id: account_id.clone(),
            reason: "Spamming".into(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let saved = account_repo
            .identity_map
            .lock()
            .unwrap()
            .get(&account_id)
            .cloned()
            .unwrap();
        assert_eq!(*saved.state(), AccountState::Banned);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_worst_case_account_not_found() {
        let (use_case, _, _) = setup();

        let cmd = BanCommand {
            account_id: AccountId::new(),
            reason: "No matter".into(),
        };

        let result = use_case.execute(cmd).await;

        // On vérifie que l'erreur NotFound remonte correctement
        assert!(matches!(
            result,
            Err(DomainError::NotFound {
                entity: "Account",
                ..
            })
        ));
    }

    #[tokio::test]
    async fn test_worst_case_concurrency_exhaustion() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        account_repo.insert(
            AccountIdentity::builder(
                account_id.clone(),
                region.clone(),
                Email::try_new("a@b.com").unwrap(),
                ExternalId::from_raw("ext"),
            )
            .build(),
        );

        // Simulation d'un conflit de version PERMANENT (ex: DB lock)
        *account_repo.error_to_return.lock().unwrap() = Some(DomainError::ConcurrencyConflict {
            reason: "High contention".into(),
        });

        let cmd = BanCommand {
            account_id,
            reason: "Ban".into(),
        };

        let result = use_case.execute(cmd).await;

        // Le retry doit finir par abandonner et rendre l'erreur
        assert!(matches!(
            result,
            Err(DomainError::ConcurrencyConflict { .. })
        ));
    }

    #[tokio::test]
    async fn test_worst_case_atomic_outbox_failure_propagation() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        account_repo.insert(
            AccountIdentity::builder(
                account_id.clone(),
                region.clone(),
                Email::try_new("a@b.com").unwrap(),
                ExternalId::from_raw("ext"),
            )
            .build(),
        );

        // On simule une erreur lors de l'écriture de l'outbox (transaction fail)
        *outbox_repo.force_error.lock().unwrap() = Some(DomainError::Internal("Disk full".into()));

        let cmd = BanCommand {
            account_id,
            reason: "Ban".into(),
        };

        let result = use_case.execute(cmd).await;

        // Le Use Case doit échouer et l'erreur de l'outbox doit remonter
        assert!(matches!(result, Err(DomainError::Internal(m)) if m == "Disk full"));
    }
}
