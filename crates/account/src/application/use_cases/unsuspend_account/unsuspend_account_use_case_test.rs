#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use crate::domain::account::builders::AccountBuilder;
    use crate::domain::account::entities::Account;
    use crate::domain::value_objects::{AccountState, Email, ExternalId, Locale};
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::use_cases::unsuspend_account::{UnsuspendAccountCommand, UnsuspendAccountUseCase};
    use crate::domain::repositories::AccountRepositoryStub;

    fn setup() -> (UnsuspendAccountUseCase, Arc<AccountRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let account_repo = Arc::new(AccountRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = UnsuspendAccountUseCase::new(account_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, account_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_unsuspend_account_success() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // 1. Arrange : On crée un compte et on le suspend (Version passe à 2)
        let mut account = Account::builder(
            account_id.clone(), 
            region.clone(),
            Email::try_new("temp@test.com").unwrap(),
            ExternalId::from_raw("ext_unsuspend")
        ).build();

        account.suspend(&region, "Suspicious activity".into()).unwrap();
        account.pull_events(); // On vide les events du setup
        let version_suspended = account.version();
        
        account_repo.add_account(account);

        let cmd = UnsuspendAccountCommand {
            account_id: account_id.clone(),
            region_code: region,
        };

        // 2. Act : On s'attend à recevoir l'Account réactivé
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok(), "La levée de suspension devrait réussir");
        let updated = result.unwrap();

        assert_eq!(*updated.state(), AccountState::Active);
        assert_eq!(updated.version(), version_suspended + 1, "La version doit être incrémentée");

        // 4. Persistence réelle
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(*saved.state(), AccountState::Active);
        
        // 5. Outbox
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1, "Un événement AccountUnsuspended attendu");
    }

    #[tokio::test]
    async fn test_unsuspend_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        let account = AccountBuilder::restore(
            account_id.clone(), region.clone(), ExternalId::from_raw("ext"),
            Email::try_new("active@test.com").unwrap(), true, None, false,
            AccountState::Active,
            None, Locale::default(),
            1, chrono::Utc::now(), chrono::Utc::now(), None
        );
        account_repo.add_account(account);

        let cmd = UnsuspendAccountCommand { account_id: account_id.clone(), region_code: region };

        let result = use_case.execute(cmd).await.unwrap();

        assert_eq!(result.version(), 1);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_unsuspend_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();

        account_repo.add_account(Account::builder(
            account_id.clone(), actual_region,
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = UnsuspendAccountCommand {
            account_id,
            region_code: RegionCode::try_new("us").unwrap(), // Mismatch
        };

        let result = use_case.execute(cmd).await;

        // Sécurité Shard : renvoie Forbidden
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }
}