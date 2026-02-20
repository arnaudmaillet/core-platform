#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::domain::entities::Account;
    use crate::domain::value_objects::{AccountState, Email, ExternalId};
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::use_cases::verify_email::{VerifyEmailCommand, VerifyEmailUseCase};
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

        // Arrange : Compte Pending
        let account = Account::builder(
            account_id.clone(),
            region.clone(),
            Username::try_new("new_user").unwrap(),
            Email::try_new("verify@test.com").unwrap(),
            ExternalId::from_raw("ext_999")
        ).build();
        account_repo.add_account(account);

        let cmd = VerifyEmailCommand {
            account_id: account_id.clone(),
            region_code: region,
            token: "valid_secure_token_123".into(),
        };

        // Act : Doit renvoyer Ok(true)
        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Ok(true)));

        // Assert
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert!(saved.is_email_verified());
        assert_eq!(*saved.state(), AccountState::Active);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_verify_email_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // Arrange : Compte déjà vérifié
        let mut account = Account::builder(
            account_id.clone(),
            region.clone(),
            Username::try_new("already_verified").unwrap(),
            Email::try_new("ok@test.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build();

        // On simule une vérification passée avec le nouveau contrat
        account.verify_email(&region).unwrap();
        account.pull_events();
        account_repo.add_account(account);

        let cmd = VerifyEmailCommand {
            account_id: account_id.clone(),
            region_code: region,
            token: "any_token".into(),
        };

        // Act : Doit renvoyer Ok(false) car déjà vérifié
        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Ok(false)));

        // Assert : Pas de double event
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_verify_email_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();

        account_repo.add_account(Account::builder(
            account_id.clone(),
            actual_region,
            Username::try_new("user").unwrap(),
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