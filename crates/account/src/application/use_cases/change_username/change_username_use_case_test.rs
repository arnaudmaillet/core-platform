#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::domain::entities::Account;
    use crate::domain::value_objects::{Email, ExternalId};
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, Username, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::use_cases::change_username::{ChangeUsernameCommand, ChangeUsernameUseCase};
    use crate::domain::repositories::AccountRepositoryStub;

    fn setup() -> (ChangeUsernameUseCase, Arc<AccountRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let account_repo = Arc::new(AccountRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = ChangeUsernameUseCase::new(account_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, account_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_change_username_success() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        account_repo.add_account(Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("old_user").unwrap(),
            Email::try_new("test@test.com").unwrap(),
            ExternalId::from_raw("ext_123")
        ).build());

        let new_username = Username::try_new("new_awesome_user").unwrap();
        let cmd = ChangeUsernameCommand {
            account_id: account_id.clone(),
            region_code: region,
            new_username: new_username.clone(),
        };

        // 1. Act : Doit renvoyer Ok(true)
        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Ok(true)));

        // 2. Assert : Vérifier la persistance
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.username(), &new_username);
        assert_eq!(saved.version(), 2);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_change_username_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let username = Username::try_new("constant_user").unwrap();

        account_repo.add_account(Account::builder(
            account_id.clone(), region.clone(),
            username.clone(), Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = ChangeUsernameCommand { account_id: account_id.clone(), region_code: region, new_username: username };

        // 1. Act : Doit renvoyer Ok(false)
        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Ok(false)));

        // 2. Assert : Aucune mutation, aucun événement
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.version(), 1);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_change_username_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();

        account_repo.add_account(Account::builder(
            account_id.clone(), actual_region,
            Username::try_new("user_eu").unwrap(), Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = ChangeUsernameCommand {
            account_id,
            region_code: RegionCode::try_new("us").unwrap(), // Region pirate
            new_username: Username::try_new("new_name").unwrap(),
        };

        let result = use_case.execute(cmd).await;

        // L'entité renvoie Forbidden via ensure_region_match
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_change_username_forbidden_when_banned() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        let mut account = Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("bad_user").unwrap(), Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build();

        account.ban(&region, "Violation".into()).unwrap();
        account_repo.add_account(account);

        let cmd = ChangeUsernameCommand {
            account_id,
            region_code: region,
            new_username: Username::try_new("new_start").unwrap(),
        };

        let result = use_case.execute(cmd).await;
        // Interdit de changer de nom quand on est banni
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_change_username_conflict_already_taken() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        account_repo.add_account(Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("original_user").unwrap(), Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let taken_name = "already_taken";
        *account_repo.error_to_return.lock().unwrap() = Some(DomainError::AlreadyExists {
            entity: "User",
            field: "username",
            value: taken_name.to_string(),
        });

        let cmd = ChangeUsernameCommand {
            account_id,
            region_code: region,
            new_username: Username::try_new(taken_name).unwrap(),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::AlreadyExists { field, .. }) if field == "username"));
    }
}