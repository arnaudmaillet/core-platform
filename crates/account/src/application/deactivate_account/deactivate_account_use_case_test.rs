#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::domain::entities::Account;
    use crate::domain::value_objects::{AccountState, Email, ExternalId};
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, Username, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::deactivate_account::{DeactivateAccountCommand, DeactivateAccountUseCase};
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

        // Arrange : Compte actif (Pending par défaut via builder)
        account_repo.add_account(Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("user_to_quit").unwrap(),
            Email::try_new("bye@test.com").unwrap(),
            ExternalId::from_raw("ext_123")
        ).build());

        let cmd = DeactivateAccountCommand {
            account_id: account_id.clone(),
            region_code: region,
        };

        // Act : Doit renvoyer Ok(true)
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Ok(true)));
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(*saved.state(), AccountState::Deactivated);
        assert_eq!(saved.version(), 2);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_deactivate_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        let mut account = Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("already_gone").unwrap(),
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build();

        // On le désactive une première fois
        account.deactivate(&region).unwrap();
        account_repo.add_account(account);

        let cmd = DeactivateAccountCommand { account_id: account_id.clone(), region_code: region };

        // Act : Doit renvoyer Ok(false)
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Ok(false)));
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.version(), 2, "La version ne doit pas changer si déjà désactivé");
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_deactivate_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();

        account_repo.add_account(Account::builder(
            account_id.clone(), actual_region,
            Username::try_new("user_eu").unwrap(), Email::try_new("a@b.com").unwrap(),
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