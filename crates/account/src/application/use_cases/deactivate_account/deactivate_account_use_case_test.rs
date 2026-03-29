#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use crate::domain::entities::Account;
    use crate::domain::value_objects::{AccountState, Email, ExternalId};
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::use_cases::deactivate_account::{DeactivateAccountCommand, DeactivateAccountUseCase};
    use crate::domain::repositories::AccountRepositoryStub;

    fn setup() -> (DeactivateAccountUseCase, Arc<AccountRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let account_repo = Arc::new(AccountRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = DeactivateAccountUseCase::new(account_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, account_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_deactivate_account_success() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // 1. Arrange : Compte initial (Version 1 par défaut)
        account_repo.add_account(Account::builder(
            account_id.clone(), 
            region.clone(),
            Email::try_new("bye@test.com").unwrap(),
            ExternalId::from_raw("ext_123")
        ).build());

        let cmd = DeactivateAccountCommand {
            account_id: account_id.clone(),
            region_code: region,
        };

        // 2. Act : On récupère l'Account
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        
        assert_eq!(*updated.state(), AccountState::Deactivated);
        assert_eq!(updated.version(), 2);

        let saved_in_db = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved_in_db.version(), 2);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_deactivate_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        let mut account = Account::builder(
            account_id.clone(), 
            region.clone(),
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build();

        // On le désactive MANUELLEMENT : la version passe à 2 (si ton entité gère l'auto-incrément)
        account.deactivate(&region).unwrap(); 
        account_repo.add_account(account);

        let cmd = DeactivateAccountCommand { 
            account_id: account_id.clone(), 
            region_code: region 
        };

        // 1. Act
        let result = use_case.execute(cmd).await;

        // 2. Assert
        assert!(result.is_ok());
        let returned = result.unwrap();

        // On vérifie que la version est restée la même que celle insérée (2)
        assert_eq!(returned.version(), 2);
        
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.version(), 2);
        
        // Aucun événement supplémentaire
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_deactivate_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();

        account_repo.add_account(Account::builder(
            account_id.clone(), actual_region,
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = DeactivateAccountCommand {
            account_id,
            region_code: RegionCode::try_new("us").unwrap(), // Mismatch
        };

        let result = use_case.execute(cmd).await;

        // Le check de région de l'entité renvoie Forbidden
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_deactivate_not_found() {
        let (use_case, _, _) = setup();
        let cmd = DeactivateAccountCommand {
            account_id: AccountId::new(),
            region_code: RegionCode::try_new("eu").unwrap(),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}