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
    use crate::application::suspend_account::{SuspendAccountCommand, SuspendAccountUseCase};
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
        let region = RegionCode::from_raw("eu");

        // Arrange : Un compte actif normal
        account_repo.add_account(Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("investigated_user").unwrap(),
            Email::try_new("check@test.com").unwrap(),
            ExternalId::from_raw("ext_789")
        ).build());

        let cmd = SuspendAccountCommand {
            account_id: account_id.clone(),
            region_code: region,
            reason: "Under investigation for fraud".into(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(*saved.state(), AccountState::Suspended);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_suspend_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // Arrange : Déjà suspendu
        let mut account = Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("already_paused").unwrap(),
            Email::try_new("p@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build();
        account.suspend("Original reason".into()).unwrap();
        account.pull_events();
        account_repo.add_account(account);

        let cmd = SuspendAccountCommand {
            account_id,
            region_code: region,
            reason: "Second call".into(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0, "Pas d'event si déjà suspendu");
    }

    #[tokio::test]
    async fn test_suspend_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();

        account_repo.add_account(Account::builder(
            account_id.clone(), RegionCode::from_raw("eu"),
            Username::try_new("user").unwrap(), Email::try_new("u@t.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = SuspendAccountCommand {
            account_id,
            region_code: RegionCode::from_raw("us"), // Mismatch
            reason: "Wrong region".into(),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::Validation { field, .. }) if field == "region_code"));
    }
}