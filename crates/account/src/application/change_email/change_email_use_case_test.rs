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
    use crate::application::change_email::{ChangeEmailCommand, ChangeEmailUseCase};
    use crate::domain::repositories::AccountRepositoryStub;

    fn setup() -> (ChangeEmailUseCase, Arc<AccountRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let account_repo = Arc::new(AccountRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = ChangeEmailUseCase::new(account_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, account_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_change_email_success() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let old_email = Email::try_new("old@test.com").unwrap();
        let new_email = Email::try_new("new@test.com").unwrap();
        let region = RegionCode::from_raw("eu");

        account_repo.add_account(Account::builder(
            account_id.clone(), RegionCode::from_raw("eu"),
            Username::try_new("user1").unwrap(), old_email,
            ExternalId::from_raw("ext_1")
        ).build());

        let cmd = ChangeEmailCommand {
            account_id: account_id.clone(),
            region_code: region, // Région correcte
            new_email: Email::try_new("new@test.com").unwrap(),
        };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.email(), &new_email);
        assert!(!saved.is_email_verified(), "L'email doit repasser en non-vérifié après changement");
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_change_email_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let email = Email::try_new("same@test.com").unwrap();
        let region = RegionCode::from_raw("eu");

        account_repo.add_account(Account::builder(
            account_id.clone(), RegionCode::from_raw("eu"),
            Username::try_new("user1").unwrap(), email.clone(),
            ExternalId::from_raw("ext_1")
        ).build());

        let cmd = ChangeEmailCommand { account_id: account_id.clone(), region_code: region, new_email: email };
        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.version(), 1, "La version ne doit pas augmenter si l'email est identique");
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_change_email_conflict_already_exists() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let taken_email = Email::try_new("taken@test.com").unwrap();
        let region = RegionCode::from_raw("eu");

        // On crée le compte cible
        account_repo.add_account(Account::builder(
            account_id.clone(), RegionCode::from_raw("eu"),
            Username::try_new("user1").unwrap(), Email::try_new("original@test.com").unwrap(),
            ExternalId::from_raw("ext_1")
        ).build());

        // On simule une erreur de contrainte unique (l'email est déjà pris en DB)
        *account_repo.error_to_return.lock().unwrap() = Some(DomainError::AlreadyExists {
            entity: "User",
            field: "email",
            value: taken_email.to_string(),
        });

        let cmd = ChangeEmailCommand { account_id, region_code: region, new_email: taken_email };
        let result = use_case.execute(cmd).await;

        assert!(matches!(result, Err(DomainError::AlreadyExists { field, .. }) if field == "email"));
    }

    #[tokio::test]
    async fn test_change_email_forbidden_when_restricted() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        let mut account = Account::builder(
            account_id.clone(), RegionCode::from_raw("eu"),
            Username::try_new("user1").unwrap(), Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext_1")
        ).build();
        account.ban("Violation".into()).unwrap();
        account_repo.add_account(account);

        let cmd = ChangeEmailCommand {
            account_id,
            region_code: region,
            new_email: Email::try_new("new@b.com").unwrap(),
        };

        let result = use_case.execute(cmd).await;
        // Ton entité Account renvoie Forbidden si on change l'email d'un banni
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_change_email_not_found() {
        let (use_case, _, _) = setup();
        let region = RegionCode::from_raw("eu");
        let cmd = ChangeEmailCommand {
            account_id: AccountId::new(),
            region_code: region,
            new_email: Email::try_new("any@test.com").unwrap(),
        };

        let result = use_case.execute(cmd).await;
        // Ici on check "Account" ou "User" selon ton stub
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_worst_case_concurrency_conflict() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        account_repo.add_account(Account::builder(
            account_id.clone(), RegionCode::from_raw("eu"),
            Username::try_new("user1").unwrap(), Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext_1")
        ).build());

        // Concurrence : quelqu'un a modifié le compte entre la lecture et l'écriture
        *account_repo.error_to_return.lock().unwrap() = Some(DomainError::ConcurrencyConflict {
            reason: "Version mismatch".into(),
        });

        let cmd = ChangeEmailCommand { account_id, region_code: region, new_email: Email::try_new("b@c.com").unwrap() };
        let result = use_case.execute(cmd).await;

        // Le Use Case va retry puis échouer si l'erreur persiste
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }
}