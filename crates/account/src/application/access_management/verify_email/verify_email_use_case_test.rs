#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use crate::domain::account::entities::Account;
    use crate::domain::value_objects::{AccountState, Email, ExternalId};
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::access_management::verify_email::{VerifyEmailCommand, VerifyEmailUseCase};
    use crate::domain::repositories::AccountRepositoryStub;

    fn setup() -> (VerifyEmailUseCase, Arc<AccountRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let account_repo = Arc::new(AccountRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = VerifyEmailUseCase::new(account_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, account_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_verify_email_success() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // 1. Arrange : Compte initial (Non vérifié, Version 1)
        account_repo.add_account(Account::builder(
            account_id.clone(),
            region.clone(),
            Email::try_new("verify@test.com").unwrap(),
            ExternalId::from_raw("ext_999")
        ).build());

        let cmd = VerifyEmailCommand {
            account_id: account_id.clone(),
            region_code: region,
            token: "valid_secure_token_123".into(),
        };

        // 2. Act : On s'attend à recevoir l'Account mis à jour
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok(), "La vérification d'email devrait réussir");
        let updated = result.unwrap();

        assert!(updated.is_email_verified());
        assert_eq!(*updated.state(), AccountState::Active, "Le compte doit devenir actif après vérification");
        assert_eq!(updated.version(), 2, "La version doit passer à 2");

        // 4. Persistence réelle
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert!(saved.is_email_verified());
        
        // 5. Outbox
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1, "Un événement EmailVerified attendu");
    }

    #[tokio::test]
    async fn test_verify_email_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // 1. Arrange : On prépare un compte déjà vérifié (Version 2)
        let mut account = Account::builder(
            account_id.clone(),
            region.clone(),
            Email::try_new("ok@test.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build();

        account.verify_email(&region).unwrap();
        account.pull_events(); // Clear setup events
        let version_verified = account.version();
        
        account_repo.add_account(account);

        let cmd = VerifyEmailCommand {
            account_id: account_id.clone(),
            region_code: region,
            token: "any_token".into(),
        };

        // 2. Act
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let returned = result.unwrap();

        assert!(returned.is_email_verified());
        assert_eq!(returned.version(), version_verified, "La version ne doit pas augmenter");

        // 4. Outbox
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0, "Idempotence : pas de double événement");
    }

    #[tokio::test]
    async fn test_verify_email_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();

        account_repo.add_account(Account::builder(
            account_id.clone(),
            actual_region,
            Email::try_new("u@t.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = VerifyEmailCommand {
            account_id,
            region_code: RegionCode::try_new("us").unwrap(), // Mismatch
            token: "token".into(),
        };

        let result = use_case.execute(cmd).await;

        // Sécurité Shard : renvoie Forbidden
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }
}