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
    use crate::application::unsuspend_account::{UnsuspendAccountCommand, UnsuspendAccountUseCase};
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
        let region = RegionCode::from_raw("eu");

        // Arrange : On crée un compte et on le suspend
        let mut account = Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("temp_user").unwrap(),
            Email::try_new("temp@test.com").unwrap(),
            ExternalId::from_raw("ext_unsuspend")
        ).build();
        account.suspend("Suspicious activity".into()).unwrap();
        account.pull_events(); // Clear events
        account_repo.add_account(account);

        let cmd = UnsuspendAccountCommand {
            account_id: account_id.clone(),
            region_code: region,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(*saved.state(), AccountState::Active);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_unsuspend_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // Arrange : Le compte est déjà actif
        account_repo.add_account(Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("active_user").unwrap(),
            Email::try_new("active@test.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = UnsuspendAccountCommand { account_id, region_code: region };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        // L'idempotence métier (unsuspend sur un compte non-suspendu) ne produit rien
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_unsuspend_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();

        // Account en EU
        account_repo.add_account(Account::builder(
            account_id.clone(), RegionCode::from_raw("eu"),
            Username::try_new("user_eu").unwrap(), Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = UnsuspendAccountCommand {
            account_id,
            region_code: RegionCode::from_raw("us"), // Mismatch
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::Validation { field, .. }) if field == "region_code"));
    }
}