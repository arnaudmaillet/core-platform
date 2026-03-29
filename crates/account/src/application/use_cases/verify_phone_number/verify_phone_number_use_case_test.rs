#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use crate::domain::entities::Account;
    use crate::domain::value_objects::{Email, ExternalId, PhoneNumber};
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::use_cases::verify_phone_number::{VerifyPhoneNumberCommand, VerifyPhoneNumberUseCase};
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
        let region = RegionCode::try_new("eu").unwrap();
        let phone = PhoneNumber::try_new("+33612345678").unwrap();

        // 1. Arrange : Compte avec téléphone non vérifié (Version 1)
        let account = Account::builder(
            account_id.clone(),
            region.clone(),
            Email::try_new("alex@test.com").unwrap(),
            ExternalId::from_raw("ext_555")
        )
        .with_phone(phone)
        .build();

        assert!(!account.is_phone_verified());
        account_repo.add_account(account);

        let cmd = VerifyPhoneNumberCommand {
            account_id: account_id.clone(),
            region_code: region,
            code: "123456".into(),
        };

        // 2. Act : On récupère l'Account mis à jour
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok(), "La vérification du téléphone devrait réussir");
        let updated = result.unwrap();

        assert!(updated.is_phone_verified());
        assert_eq!(updated.version(), 2, "La version doit être passée à 2");

        // 4. Persistence réelle
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert!(saved.is_phone_verified());
        
        // 5. Outbox
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1, "Un événement PhoneVerified attendu");
    }

    #[tokio::test]
    async fn test_verify_phone_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // 1. Arrange : On simule un téléphone déjà vérifié (Version 2)
        let mut account = Account::builder(
            account_id.clone(),
            region.clone(),
            Email::try_new("s@test.com").unwrap(),
            ExternalId::from_raw("ext")
        )
        .with_phone(PhoneNumber::try_new("+33600000000").unwrap())
        .build();

        account.verify_phone(&region).unwrap();
        account.pull_events(); // On vide les événements du setup
        let version_verified = account.version();
        
        account_repo.add_account(account);

        let cmd = VerifyPhoneNumberCommand {
            account_id: account_id.clone(),
            region_code: region,
            code: "000000".into(),
        };

        // 2. Act
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let returned = result.unwrap();

        assert!(returned.is_phone_verified());
        assert_eq!(returned.version(), version_verified, "La version ne doit pas augmenter");

        // 4. Outbox
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0, "Idempotence : aucun événement généré");
    }

    #[tokio::test]
    async fn test_verify_phone_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();

        account_repo.add_account(Account::builder(
            account_id.clone(),
            actual_region,
            Email::try_new("u@t.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = VerifyPhoneNumberCommand {
            account_id,
            region_code: RegionCode::try_new("us").unwrap(), // Mismatch
            code: "111111".into(),
        };

        let result = use_case.execute(cmd).await;

        // Sécurité Shard : Forbidden
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }
}