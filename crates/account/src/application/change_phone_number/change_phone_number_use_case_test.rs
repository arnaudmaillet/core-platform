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
    use crate::application::change_phone_number::change_phone_number_command::ChangePhoneNumberCommand;
    use crate::application::change_phone_number::change_phone_number_use_case::ChangePhoneNumberUseCase;
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
        let region = RegionCode::from_raw("eu");
        let old_phone = PhoneNumber::try_new("+33612345678").unwrap();
        let new_phone = PhoneNumber::try_new("+33687654321").unwrap();

        let mut account = Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("user1").unwrap(),
            Email::try_new("test@test.com").unwrap(),
            ExternalId::from_raw("ext_1")
        ).build();
        account.change_phone_number(old_phone).unwrap();
        account_repo.add_account(account);

        let cmd = ChangePhoneNumberCommand {
            account_id: account_id.clone(),
            region_code: region,
            new_phone: new_phone.clone(),
        };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.phone_number(), Some(&new_phone));
        assert!(!saved.is_phone_verified(), "Le téléphone doit être dé-vérifié");
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_change_phone_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();

        // Compte en EU
        account_repo.add_account(Account::builder(
            account_id.clone(), RegionCode::from_raw("eu"),
            Username::try_new("user1").unwrap(), Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        // Commande ciblant US
        let cmd = ChangePhoneNumberCommand {
            account_id,
            region_code: RegionCode::from_raw("us"),
            new_phone: PhoneNumber::try_new("+1555000111").unwrap(),
        };

        let result = use_case.execute(cmd).await;

        // Si tu n'as pas encore ajouté le check dans ton code, ce test va échouer.
        // C'est un garde-fou essentiel pour le sharding.
        assert!(matches!(result, Err(DomainError::Validation { field, .. }) if field == "region_code"));
    }

    #[tokio::test]
    async fn test_change_phone_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let phone = PhoneNumber::try_new("+33600000000").unwrap();

        let mut account = Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("user1").unwrap(), Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build();
        account.change_phone_number(phone.clone()).unwrap(); // Version -> 2
        account_repo.add_account(account);

        let cmd = ChangePhoneNumberCommand { account_id, region_code: region, new_phone: phone };
        let result = use_case.execute(cmd.clone()).await;

        assert!(result.is_ok());
        let saved = account_repo.accounts.lock().unwrap().get(&cmd.account_id).cloned().unwrap();
        assert_eq!(saved.version(), 2, "La version ne doit pas changer si le numéro est identique");
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_worst_case_outbox_failure_propagation() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

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