#[cfg(test)]
mod tests {
    use crate::application::lifecycle::unsuspend::{UnsuspendCommand, UnsuspendUseCase};
    use crate::domain::account::builders::AccountIdentityBuilder;
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::repositories::AccountIdentityRepositoryStub;
    use crate::domain::value_objects::{AccountState, Email, ExternalId, Locale};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;
    use std::sync::Arc;

    fn setup() -> (
        UnsuspendUseCase,
        Arc<AccountIdentityRepositoryStub>,
        Arc<OutboxRepositoryStub>,
    ) {
        let account_repo = Arc::new(AccountIdentityRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = UnsuspendUseCase::new(account_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, account_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_unsuspend_account_success() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // 1. Arrange : On crée un compte et on le suspend (Version passe à 2)
        let mut account = AccountIdentity::builder(
            account_id.clone(),
            region.clone(),
            Email::try_new("temp@test.com").unwrap(),
            ExternalId::from_raw("ext_unsuspend"),
        )
        .build();

        account
            .suspend("Suspicious activity".into())
            .unwrap();
        account.pull_events(); // On vide les events du setup
        let version_suspended = account.version();

        account_repo.add_account(account);

        let cmd = UnsuspendCommand {
            account_id: account_id.clone(),
        };

        // 2. Act : On s'attend à recevoir l'Account réactivé
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok(), "La levée de suspension devrait réussir");
        let updated = result.unwrap();

        assert_eq!(*updated.state(), AccountState::Active);
        assert_eq!(
            updated.version(),
            version_suspended + 1,
            "La version doit être incrémentée"
        );

        // 4. Persistence réelle
        let saved = account_repo
            .identity_map
            .lock()
            .unwrap()
            .get(&account_id)
            .cloned()
            .unwrap();
        assert_eq!(*saved.state(), AccountState::Active);

        // 5. Outbox
        assert_eq!(
            outbox_repo.saved_events.lock().unwrap().len(),
            1,
            "Un événement AccountUnsuspended attendu"
        );
    }

    #[tokio::test]
    async fn test_unsuspend_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        let account = AccountIdentityBuilder::restore(
            account_id.clone(),
            region.clone(),
            ExternalId::from_raw("ext"),
            Email::try_new("active@test.com").unwrap(),
            true,
            None,
            false,
            AccountState::Active,
            None,
            Locale::default(),
            1,
            chrono::Utc::now(),
            chrono::Utc::now(),
            None,
        );
        account_repo.add_account(account);

        let cmd = UnsuspendCommand {
            account_id: account_id.clone(),
        };

        let result = use_case.execute(cmd).await.unwrap();

        assert_eq!(result.version(), 1);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

}
