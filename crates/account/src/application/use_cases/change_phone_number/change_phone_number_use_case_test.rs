#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::domain::entities::Account;
    use crate::domain::value_objects::{Email, ExternalId, PhoneNumber};
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, Username, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::use_cases::change_phone_number::change_phone_number_command::ChangePhoneNumberCommand;
    use crate::application::use_cases::change_phone_number::change_phone_number_use_case::ChangePhoneNumberUseCase;
    use crate::domain::repositories::AccountRepositoryStub;

    fn setup() -> (ChangePhoneNumberUseCase, Arc<AccountRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let account_repo = Arc::new(AccountRepositoryStub::new());
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

        let mut account = Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("user1").unwrap(),
            Email::try_new("test@test.com").unwrap(),
            ExternalId::from_raw("ext_1")
        ).build();

        // Setup initial avec l'ancienne signature ou via restore pour simuler l'état existant
        account.change_phone_number(&region, old_phone).unwrap();
        account_repo.add_account(account);

        let cmd = ChangePhoneNumberCommand {
            account_id: account_id.clone(),
            region_code: region,
            new_phone: new_phone.clone(),
        };

        // 1. Act : Doit renvoyer Ok(true)
        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Ok(true)));

        // 2. Assert : Vérifier l'état et les conséquences métier
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.phone_number(), Some(&new_phone));
        assert!(!saved.is_phone_verified(), "Le téléphone doit être dé-vérifié après changement");
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_change_phone_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();

        account_repo.add_account(Account::builder(
            account_id.clone(), actual_region,
            Username::try_new("user1").unwrap(), Email::try_new("a@b.com").unwrap(),
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

        let mut account = Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("user1").unwrap(), Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build();

        account.change_phone_number(&region, phone.clone()).unwrap(); // Version -> 2
        account_repo.add_account(account);

        let cmd = ChangePhoneNumberCommand { account_id: account_id.clone(), region_code: region, new_phone: phone };

        // 1. Act : Doit renvoyer Ok(false)
        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Ok(false)));

        // 2. Assert : Pas de double save, pas d'event
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.version(), 2);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_worst_case_outbox_failure_propagation() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        account_repo.add_account(Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("user1").unwrap(), Email::try_new("a@b.com").unwrap(),
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