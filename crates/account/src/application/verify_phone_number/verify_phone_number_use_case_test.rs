#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::domain::entities::Account;
    use crate::domain::value_objects::{Email, ExternalId, PhoneNumber};
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::verify_phone_number::{VerifyPhoneNumberCommand, VerifyPhoneNumberUseCase};
    use crate::domain::repositories::AccountRepositoryStub;

    fn setup() -> (VerifyPhoneNumberUseCase, Arc<AccountRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let account_repo = Arc::new(AccountRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = VerifyPhoneNumberUseCase::new(account_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, account_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_verify_phone_success() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // ✅ Arrange : Compte avec un téléphone non vérifié
        let account = Account::builder(
            account_id.clone(),
            region.clone(),
            Username::try_new("alex_phone").unwrap(),
            Email::try_new("alex@test.com").unwrap(),
            ExternalId::from_raw("ext_555")
        )
            .with_phone(PhoneNumber::try_new("+33612345678").unwrap())
            .build();

        assert!(!account.is_phone_verified());
        account_repo.add_account(account);

        let cmd = VerifyPhoneNumberCommand {
            account_id: account_id.clone(),
            region_code: region,
            code: "123456".into(), // Le code OTP simulé
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert!(saved.is_phone_verified());
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_verify_phone_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // ✅ Arrange : Compte déjà vérifié
        let mut account = Account::builder(
            account_id.clone(),
            region.clone(),
            Username::try_new("secure_user").unwrap(),
            Email::try_new("s@test.com").unwrap(),
            ExternalId::from_raw("ext")
        )
            .with_phone(PhoneNumber::try_new("+33600000000").unwrap())
            .build();

        account.verify_phone();
        account.pull_events();
        account_repo.add_account(account);

        let cmd = VerifyPhoneNumberCommand {
            account_id,
            region_code: region,
            code: "000000".into(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        // Pas d'événement produit car l'état n'a pas changé
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_verify_phone_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();

        account_repo.add_account(Account::builder(
            account_id.clone(),
            RegionCode::from_raw("eu"),
            Username::try_new("user").unwrap(),
            Email::try_new("u@t.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = VerifyPhoneNumberCommand {
            account_id,
            region_code: RegionCode::from_raw("us"), // 👈 Mismatch de shard
            code: "111111".into(),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::Validation { field, .. }) if field == "region_code"));
    }
}