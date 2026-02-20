#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use crate::domain::entities::Account;
    use crate::domain::value_objects::{AccountState, Email, ExternalId};
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, Username, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::use_cases::ban_account::{BanAccountCommand, BanAccountUseCase};
    use crate::domain::repositories::AccountRepositoryStub;

    fn setup() -> (BanAccountUseCase, Arc<AccountRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let account_repo = Arc::new(AccountRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = BanAccountUseCase::new(account_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, account_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_ban_account_success() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // Création d'un compte actif
        let account = Account::builder(
            account_id.clone(),
            region.clone(),
            Username::try_new("bad_user").unwrap(),
            Email::try_new("user@example.com").unwrap(),
            ExternalId::from_raw("ext_123")
        ).build();
        account_repo.add_account(account);

        let cmd = BanAccountCommand {
            account_id: account_id.clone(),
            region_code: region.clone(),
            reason: "TOS Violation".into(),
        };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(*saved.state(), AccountState::Banned);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_ban_account_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();

        let account = Account::builder(
            account_id.clone(),
            RegionCode::from_raw("eu"), // Compte en EU
            Username::try_new("user").unwrap(),
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build();
        account_repo.add_account(account);

        let cmd = BanAccountCommand {
            account_id,
            region_code: RegionCode::from_raw("us"), // Commande cible US
            reason: "Wrong region".into(),
        };

        let result = use_case.execute(cmd).await;

        // Doit échouer car on tente de bannir un compte sur la mauvaise région (shard)
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_ban_account_idempotency_no_double_ban() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        let mut account = Account::builder(
            account_id.clone(),
            region.clone(),
            Username::try_new("user").unwrap(),
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build();

        account.ban(&region, "First reason".into()).unwrap();
        account_repo.add_account(account);

        let cmd = BanAccountCommand {
            account_id,
            region_code: region,
            reason: "Second attempt".into(),
        };

        let result = use_case.execute(cmd.clone()).await;

        assert!(result.is_ok());
        let saved = account_repo.accounts.lock().unwrap().get(&cmd.account_id).unwrap().clone();

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
        account_repo.add_account(Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("troll").unwrap(),
            Email::try_new("troll@internet.com").unwrap(),
            ExternalId::from_raw("ext_1")
        ).build());

        let cmd = BanAccountCommand {
            account_id: account_id.clone(),
            region_code: region,
            reason: "Spamming".into(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(*saved.state(), AccountState::Banned);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_worst_case_account_not_found() {
        let (use_case, _, _) = setup();

        let cmd = BanAccountCommand {
            account_id: AccountId::new(),
            region_code: RegionCode::from_raw("eu"),
            reason: "No matter".into(),
        };

        let result = use_case.execute(cmd).await;

        // On vérifie que l'erreur NotFound remonte correctement
        assert!(matches!(result, Err(DomainError::NotFound { entity: "Account", .. })));
    }

    #[tokio::test]
    async fn test_worst_case_concurrency_exhaustion() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        account_repo.add_account(Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("user").unwrap(),
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        // Simulation d'un conflit de version PERMANENT (ex: DB lock)
        *account_repo.error_to_return.lock().unwrap() = Some(DomainError::ConcurrencyConflict {
            reason: "High contention".into(),
        });

        let cmd = BanAccountCommand { account_id, region_code: region, reason: "Ban".into() };

        let result = use_case.execute(cmd).await;

        // Le retry doit finir par abandonner et rendre l'erreur
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }

    #[tokio::test]
    async fn test_worst_case_atomic_outbox_failure_propagation() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        account_repo.add_account(Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("user").unwrap(),
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        // On simule une erreur lors de l'écriture de l'outbox (transaction fail)
        *outbox_repo.force_error.lock().unwrap() = Some(DomainError::Internal("Disk full".into()));

        let cmd = BanAccountCommand { account_id, region_code: region, reason: "Ban".into() };

        let result = use_case.execute(cmd).await;

        // Le Use Case doit échouer et l'erreur de l'outbox doit remonter
        assert!(matches!(result, Err(DomainError::Internal(m)) if m == "Disk full"));
    }
}