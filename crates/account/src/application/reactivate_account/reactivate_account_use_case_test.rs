#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use chrono::Utc;
    use crate::domain::entities::Account;
    use crate::domain::value_objects::{AccountState, Email, ExternalId, Locale};
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, Username, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::reactivate_account::{ReactivateAccountCommand, ReactivateAccountUseCase};
    use crate::domain::builders::AccountBuilder;
    use crate::domain::repositories::AccountRepositoryStub;

    fn setup() -> (ReactivateAccountUseCase, Arc<AccountRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let account_repo = Arc::new(AccountRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = ReactivateAccountUseCase::new(account_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, account_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_reactivate_account_success() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // Arrange : On crée un compte et on le désactive
        let mut account = Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("user_back").unwrap(),
            Email::try_new("back@test.com").unwrap(),
            ExternalId::from_raw("ext_123")
        ).build();
        account.deactivate().unwrap();
        account.pull_events(); // Clear events
        account_repo.add_account(account);

        let cmd = ReactivateAccountCommand {
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
    async fn test_reactivate_forbidden_if_banned() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // Arrange : Compte Banni
        let mut account = Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("bad_user").unwrap(),
            Email::try_new("banned@test.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build();
        account.ban("Violation".into()).unwrap();
        account_repo.add_account(account);

        let cmd = ReactivateAccountCommand { account_id, region_code: region };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert : Doit échouer car seul Deactivated peut être réactivé manuellement
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_reactivate_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // On utilise restore pour créer un compte déjà ACTIVE
        let account = AccountBuilder::restore(
            account_id.clone(),
            region.clone(),
            ExternalId::from_raw("ext"),
            Username::try_new("always_here").unwrap(),
            Email::try_new("a@b.com").unwrap(),
            true,
            None, // phone
            false,
            AccountState::Active,
            None,
            Locale::default(),
            1, // version
            Utc::now(), Utc::now(), Some(Utc::now())
        );

        account_repo.add_account(account);

        let cmd = ReactivateAccountCommand { account_id, region_code: region };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok(), "La réactivation d'un compte déjà actif doit être un succès (idempotence)");
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_reactivate_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();

        account_repo.add_account(Account::builder(
            account_id.clone(), RegionCode::from_raw("eu"),
            Username::try_new("user_eu").unwrap(), Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = ReactivateAccountCommand {
            account_id,
            region_code: RegionCode::from_raw("us"), // Erreur de région
        };

        // ⚠️ Vérifie que tu as bien ajouté le check de région dans try_execute_once !
        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::Validation { field, .. }) if field == "region_code"));
    }
}