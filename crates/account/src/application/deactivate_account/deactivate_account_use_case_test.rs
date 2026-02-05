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
        let region = RegionCode::from_raw("eu");

        // Arrange : Compte actif
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

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(*saved.state(), AccountState::Deactivated);
        assert_eq!(saved.version(), 2);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_deactivate_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        let mut account = Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("already_gone").unwrap(),
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build();

        account.deactivate().unwrap(); // Passage en Deactivated, version 2
        account_repo.add_account(account);

        let cmd = DeactivateAccountCommand { account_id, region_code: region };

        // Act
        let result = use_case.execute(cmd.clone()).await;

        // Assert
        assert!(result.is_ok());
        let saved = account_repo.accounts.lock().unwrap().get(&cmd.account_id).cloned().unwrap();
        assert_eq!(saved.version(), 2, "La version ne doit pas changer si déjà désactivé");
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_deactivate_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();

        account_repo.add_account(Account::builder(
            account_id.clone(), RegionCode::from_raw("eu"),
            Username::try_new("user_eu").unwrap(), Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = DeactivateAccountCommand {
            account_id,
            region_code: RegionCode::from_raw("us"), // Erreur de région
        };

        let result = use_case.execute(cmd).await;

        // Ce test souligne l'importance d'ajouter le check de région dans ton Use Case
        assert!(matches!(result, Err(DomainError::Validation { field, .. }) if field == "region_code"));
    }

    #[tokio::test]
    async fn test_deactivate_not_found() {
        let (use_case, _, _) = setup();
        let cmd = DeactivateAccountCommand {
            account_id: AccountId::new(),
            region_code: RegionCode::from_raw("eu"),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}