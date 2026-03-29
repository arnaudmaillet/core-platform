#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use chrono::Utc;
    use crate::domain::entities::Account;
    use crate::domain::value_objects::{AccountState, Email, ExternalId, Locale};
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::use_cases::reactivate_account::{ReactivateAccountCommand, ReactivateAccountUseCase};
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
        let region = RegionCode::try_new("eu").unwrap();

        // 1. Arrange : On crée un compte désactivé
        let mut account = Account::builder(
            account_id.clone(), 
            region.clone(),
            Email::try_new("back@test.com").unwrap(),
            ExternalId::from_raw("ext_123")
        ).build();

        // On le passe en désactivé (Version passe à 2)
        account.deactivate(&region).unwrap();
        account.pull_events(); 
        let version_deactivated = account.version();
        
        account_repo.add_account(account);

        let cmd = ReactivateAccountCommand {
            account_id: account_id.clone(),
            region_code: region,
        };

        // 2. Act
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let updated = result.unwrap();

        assert_eq!(*updated.state(), AccountState::Active);
        assert_eq!(updated.version(), version_deactivated + 1);

        // 4. Persistence
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(*saved.state(), AccountState::Active);
        
        // 5. Outbox
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_reactivate_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // 1. Arrange : Compte déjà ACTIVE via restore
        let account = AccountBuilder::restore(
            account_id.clone(),
            region.clone(),
            ExternalId::from_raw("ext"),
            Email::try_new("a@b.com").unwrap(),
            true,
            None,
            false,
            AccountState::Active, // Déjà actif
            None,
            Locale::default(),
            1, // Version initiale
            Utc::now(), Utc::now(), Some(Utc::now())
        );

        account_repo.add_account(account);

        let cmd = ReactivateAccountCommand { 
            account_id: account_id.clone(), 
            region_code: region 
        };

        // 2. Act
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let returned = result.unwrap();

        assert_eq!(*returned.state(), AccountState::Active);
        assert_eq!(returned.version(), 1);

        // 4. Outbox
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_reactivate_forbidden_if_banned() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        let mut account = Account::builder(
            account_id.clone(), region.clone(),
            Email::try_new("banned@test.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build();

        account.ban(&region, "Violation".into()).unwrap();
        account_repo.add_account(account);

        let cmd = ReactivateAccountCommand { account_id, region_code: region };

        let result = use_case.execute(cmd).await;

        // Seul un compte Deactivated peut être réactivé manuellement
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_reactivate_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();

        account_repo.add_account(Account::builder(
            account_id.clone(), actual_region,
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = ReactivateAccountCommand {
            account_id,
            region_code: RegionCode::try_new("us").unwrap(), // Mismatch
        };

        let result = use_case.execute(cmd).await;

        // Sécurité Shard : renvoie Forbidden
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }
}