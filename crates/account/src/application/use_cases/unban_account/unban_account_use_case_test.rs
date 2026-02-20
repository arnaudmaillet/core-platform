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
    use crate::application::use_cases::unban_account::{UnbanAccountCommand, UnbanAccountUseCase};
    use crate::domain::repositories::AccountRepositoryStub;

    fn setup() -> (UnbanAccountUseCase, Arc<AccountRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let account_repo = Arc::new(AccountRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = UnbanAccountUseCase::new(account_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, account_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_unban_account_success() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // Arrange : On crée un compte banni
        let mut account = Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("rehabilitated_user").unwrap(),
            Email::try_new("clean@test.com").unwrap(),
            ExternalId::from_raw("ext_000")
        ).build();

        // On utilise la nouvelle signature avec région
        account.ban(&region, "Past violation".into()).unwrap();
        account.pull_events();
        account_repo.add_account(account);

        let cmd = UnbanAccountCommand {
            account_id: account_id.clone(),
            region_code: region,
        };

        // Act : Doit renvoyer Ok(true)
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Ok(true)));
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(*saved.state(), AccountState::Active);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_unban_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // Arrange : Compte déjà actif (donc pas banni)
        account_repo.add_account(Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("active_user").unwrap(),
            Email::try_new("active@test.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = UnbanAccountCommand { account_id: account_id.clone(), region_code: region };

        // Act : Doit renvoyer Ok(false)
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Ok(false)));
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0, "Pas d'event si le compte n'était pas banni");
    }

    #[tokio::test]
    async fn test_unban_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();

        account_repo.add_account(Account::builder(
            account_id.clone(), actual_region,
            Username::try_new("user_eu").unwrap(), Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = UnbanAccountCommand {
            account_id,
            region_code: RegionCode::try_new("us").unwrap(), // Mismatch
        };

        let result = use_case.execute(cmd).await;

        // Sécurité Shard : renvoie Forbidden
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }
}