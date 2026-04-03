#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use crate::domain::account::entities::Account;
    use crate::domain::value_objects::{AccountState, Email, ExternalId};
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::use_cases::suspend_account::{SuspendAccountCommand, SuspendAccountUseCase};
    use crate::domain::repositories::AccountRepositoryStub;

    fn setup() -> (SuspendAccountUseCase, Arc<AccountRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let account_repo = Arc::new(AccountRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = SuspendAccountUseCase::new(account_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, account_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_suspend_account_success() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // 1. Arrange : Compte actif (Version 1)
        account_repo.add_account(Account::builder(
            account_id.clone(), 
            region.clone(),
            Email::try_new("check@test.com").unwrap(),
            ExternalId::from_raw("ext_789")
        ).build());

        let cmd = SuspendAccountCommand {
            account_id: account_id.clone(),
            region_code: region,
            reason: "Under investigation for fraud".into(),
        };

        // 2. Act : On s'attend à recevoir l'Account mis à jour
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok(), "La suspension devrait réussir");
        let updated = result.unwrap();

        assert_eq!(*updated.state(), AccountState::Suspended);
        assert_eq!(updated.version(), 2, "La version doit être passée à 2");

        // 4. Persistence réelle
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(*saved.state(), AccountState::Suspended);
        
        // 5. Outbox
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1, "Un événement AccountSuspended attendu");
    }

    #[tokio::test]
    async fn test_suspend_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // 1. Arrange : On crée et on suspend manuellement
        let mut account = Account::builder(
            account_id.clone(), 
            region.clone(),
            Email::try_new("p@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build();

        account.suspend(&region, "Original reason".into()).unwrap();
        account.pull_events();
        let version_at_suspension = account.version();
        
        account_repo.add_account(account);

        let cmd = SuspendAccountCommand {
            account_id: account_id.clone(),
            region_code: region,
            reason: "Second call".into(),
        };

        // 2. Act
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let returned = result.unwrap();

        assert_eq!(*returned.state(), AccountState::Suspended);
        assert_eq!(returned.version(), version_at_suspension, "La version ne doit pas augmenter");

        // 4. Outbox
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0, "Idempotence : aucun événement produit");
    }

    #[tokio::test]
    async fn test_suspend_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();

        account_repo.add_account(Account::builder(
            account_id.clone(), actual_region,
            Email::try_new("u@t.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = SuspendAccountCommand {
            account_id,
            region_code: RegionCode::try_new("us").unwrap(), // Mismatch
            reason: "Wrong region".into(),
        };

        let result = use_case.execute(cmd).await;

        // Sécurité Shard : renvoie Forbidden
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }
}