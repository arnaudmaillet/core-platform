#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use crate::domain::account::builders::AccountIdentityBuilder;
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::value_objects::{AccountState, Email, ExternalId, Locale, PhoneNumber};
    use chrono::Utc;
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::settings::change_phone_number::change_phone_number_command::ChangePhoneNumberCommand;
    use crate::application::settings::change_phone_number::change_phone_number_use_case::ChangePhoneNumberUseCase;
    use crate::domain::repositories::AccountIdentityRepositoryStub;

    fn setup() -> (ChangePhoneNumberUseCase, Arc<AccountIdentityRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let account_repo = Arc::new(AccountIdentityRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = ChangePhoneNumberUseCase::new(account_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, account_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_change_phone_number_success() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let old_phone = PhoneNumber::try_new("+33612345678").unwrap();
        let new_phone = PhoneNumber::try_new("+33687654321").unwrap();

        // RESTORE : On simule un compte existant en v1 avec l'ancien téléphone
        let account = AccountIdentityBuilder::restore(
            account_id.clone(), region.clone(), ExternalId::from_raw("ext_1"),
            Email::try_new("test@test.com").unwrap(), true, Some(old_phone), true,
            AccountState::Active, None, Locale::default(),
            1, chrono::Utc::now(), chrono::Utc::now(), None
        );
        account_repo.add_account(account);

        let cmd = ChangePhoneNumberCommand {
            account_id: account_id.clone(),
            region_code: region,
            new_phone: new_phone.clone(),
        };

        let result = use_case.execute(cmd).await.unwrap();

        // Assert : v1 + 1 changement = v2
        assert_eq!(result.phone_number(), Some(&new_phone));
        assert_eq!(result.version(), 2); 
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_change_phone_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();

        account_repo.add_account(AccountIdentity::builder(
            account_id.clone(), actual_region,
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = ChangePhoneNumberCommand {
            account_id,
            region_code: RegionCode::try_new("us").unwrap(), // Région pirate
            new_phone: PhoneNumber::try_new("+1555000111").unwrap(),
        };

        let result = use_case.execute(cmd).await;

        // Le check ensure_region_match renvoie Forbidden
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_change_phone_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let phone = PhoneNumber::try_new("+33600000000").unwrap();

        // --- ARRANGE ---
        // On simule un compte qui a DEJÀ le téléphone en base, avec une version propre (1).
        // C'est ça que "restore" fait : il crée l'état final sans déclencher les mutations.
        let account = AccountIdentityBuilder::restore(
            account_id.clone(), region.clone(), ExternalId::from_raw("ext"),
            Email::try_new("a@b.com").unwrap(), true, Some(phone.clone()), true,
            AccountState::Active, None, Locale::default(),
            1,
            Utc::now(), Utc::now(), None
        );
        account_repo.add_account(account);

        let cmd = ChangePhoneNumberCommand { 
            account_id: account_id.clone(), 
            region_code: region, 
            new_phone: phone.clone() 
        };

        // --- ACT ---
        let result = use_case.execute(cmd).await.unwrap();

        // --- ASSERT ---
        // 1. L'objet retourné n'a pas bougé (toujours version 1)
        assert_eq!(result.phone_number(), Some(&phone));
        assert_eq!(result.version(), 1); 

        // 2. Vérification DB (ton point 3) : RIEN n'a été écrasé en base
        let saved_in_db = account_repo.identity_map.lock().unwrap()
            .get(&account_id).cloned().unwrap();
        assert_eq!(saved_in_db.version(), 1);

        // 3. Vérification Outbox (ton point 4) : AUCUN événement produit
        let events = outbox_repo.saved_events.lock().unwrap();
        assert_eq!(events.len(), 0, "L'idempotence ne doit produire aucun événement");
    }

    #[tokio::test]
    async fn test_worst_case_outbox_failure_propagation() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        account_repo.add_account(AccountIdentity::builder(
            account_id.clone(), region.clone(),
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        *outbox_repo.force_error.lock().unwrap() = Some(DomainError::Internal("Kafka/Outbox Down".into()));

        let cmd = ChangePhoneNumberCommand {
            account_id, region_code: region,
            new_phone: PhoneNumber::try_new("+33611223344").unwrap(),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::Internal(_))));
    }
}