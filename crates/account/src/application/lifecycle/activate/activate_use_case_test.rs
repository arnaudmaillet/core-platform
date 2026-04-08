#[cfg(test)]
mod tests {
    use crate::application::lifecycle::activate::{ActivateCommand, ReactivateUseCase};
    use crate::domain::account::builders::AccountIdentityBuilder;
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::repositories::AccountIdentityRepositoryStub;
    use crate::domain::value_objects::{AccountState, Email, ExternalId, Locale};
    use chrono::Utc;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;
    use std::sync::Arc;

    fn setup() -> (
        ReactivateUseCase,
        Arc<AccountIdentityRepositoryStub>,
        Arc<OutboxRepositoryStub>,
    ) {
        let account_repo = Arc::new(AccountIdentityRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case =
            ReactivateUseCase::new(account_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, account_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_reactivate_account_success() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // 1. Arrange : On crée un compte désactivé
        let mut account = AccountIdentity::builder(
            account_id.clone(),
            region.clone(),
            Email::try_new("back@test.com").unwrap(),
            ExternalId::from_raw("ext_123"),
        )
        .build();

        // On le passe en désactivé (Version passe à 2)
        account.deactivate().unwrap();
        account.pull_events();
        let version_deactivated = account.version();

        account_repo.add_account(account);

        let cmd = ActivateCommand {
            account_id: account_id.clone(),
        };

        // 2. Act
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let updated = result.unwrap();

        assert_eq!(*updated.state(), AccountState::Active);
        assert_eq!(updated.version(), version_deactivated + 1);

        // 4. Persistence
        let saved = account_repo
            .identity_map
            .lock()
            .unwrap()
            .get(&account_id)
            .cloned()
            .unwrap();
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
        let account = AccountIdentityBuilder::restore(
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
            Utc::now(),
            Utc::now(),
            Some(Utc::now()),
        );

        account_repo.add_account(account);

        let cmd = ActivateCommand {
            account_id: account_id.clone(),
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

        let mut account = AccountIdentity::builder(
            account_id.clone(),
            region.clone(),
            Email::try_new("banned@test.com").unwrap(),
            ExternalId::from_raw("ext"),
        )
        .build();

        account.ban("Violation".into()).unwrap();
        account_repo.add_account(account);

        let cmd = ActivateCommand {
            account_id,
        };

        let result = use_case.execute(cmd).await;

        // Seul un compte Deactivated peut être réactivé manuellement
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }
}
