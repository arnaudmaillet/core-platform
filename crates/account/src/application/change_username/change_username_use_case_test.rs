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
    use crate::application::change_username::{ChangeUsernameCommand, ChangeUsernameUseCase};
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
        let region = RegionCode::from_raw("eu");

        // Initialisation avec "old_user"
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

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.username(), &new_username);
        assert_eq!(saved.version(), 2);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_change_username_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let username = Username::try_new("constant_user").unwrap();

        account_repo.add_account(Account::builder(
            account_id.clone(), region.clone(),
            username.clone(), Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        // On renvoie le même username
        let cmd = ChangeUsernameCommand { account_id, region_code: region, new_username: username };
        let result = use_case.execute(cmd.clone()).await;

        assert!(result.is_ok());
        let saved = account_repo.accounts.lock().unwrap().get(&cmd.account_id).cloned().unwrap();
        assert_eq!(saved.version(), 1, "Idempotence: la version ne doit pas changer");
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_change_username_conflict_already_taken() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        account_repo.add_account(Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("original_user").unwrap(), Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        // Simulation d'un conflit UNIQUE KEY en DB lors du save()
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

    #[tokio::test]
    async fn test_change_username_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();

        account_repo.add_account(Account::builder(
            account_id.clone(), RegionCode::from_raw("eu"),
            Username::try_new("user_eu").unwrap(), Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        // Commande ciblant US pour un compte EU
        let cmd = ChangeUsernameCommand {
            account_id,
            region_code: RegionCode::from_raw("us"),
            new_username: Username::try_new("new_name").unwrap(),
        };

        let result = use_case.execute(cmd).await;
        // Si tu n'as pas encore ajouté le check de région dans try_execute_once, ce test va échouer
        // car il manque ce garde-fou vital pour le sharding.
        assert!(matches!(result, Err(DomainError::Validation { field, .. }) if field == "region_code"));
    }

    #[tokio::test]
    async fn test_change_username_forbidden_when_banned() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        let mut account = Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("bad_user").unwrap(), Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build();
        account.ban("Violation".into()).unwrap();
        account_repo.add_account(account);

        let cmd = ChangeUsernameCommand {
            account_id,
            region_code: region,
            new_username: Username::try_new("new_start").unwrap(),
        };

        let result = use_case.execute(cmd).await;
        // L'entité Account doit refuser la mutation si l'état est bloqué
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }
}