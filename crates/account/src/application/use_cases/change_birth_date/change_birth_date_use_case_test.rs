#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use chrono::{Utc, TimeZone};
    use crate::domain::entities::Account;
    use crate::domain::value_objects::{Email, ExternalId, BirthDate};
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, Username, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::use_cases::change_birth_date::{ChangeBirthDateCommand, ChangeBirthDateUseCase};
    use crate::domain::repositories::AccountRepositoryStub;

    fn setup() -> (ChangeBirthDateUseCase, Arc<AccountRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let account_repo = Arc::new(AccountRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = ChangeBirthDateUseCase::new(account_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, account_repo, outbox_repo)
    }

    fn adult_birth_date() -> BirthDate {
        let date = Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap().date_naive();
        BirthDate::try_new(date).unwrap()
    }

    // --- CAS 1 : SUCCÈS (HAPPY PATH) ---
    #[tokio::test]
    async fn test_change_birth_date_success() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        account_repo.add_account(Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("alex").unwrap(),
            Email::try_new("alex@test.com").unwrap(),
            ExternalId::from_raw("ext_1")
        ).build());

        let date_raw = Utc.with_ymd_and_hms(1995, 5, 15, 0, 0, 0).unwrap().date_naive();
        let new_date = BirthDate::try_new(date_raw).unwrap();

        let cmd = ChangeBirthDateCommand {
            account_id: account_id.clone(),
            region_code: region,
            birth_date: new_date.clone(),
        };

        // 1. On vérifie que execute renvoie Ok(true)
        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Ok(true)));

        // 2. Vérifier la persistance
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.birth_date(), Some(&new_date));
        assert_eq!(saved.version(), 2);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    // --- CAS 2 : ERREUR DE RÉGION (SÉCURITÉ SHARD) ---
    #[tokio::test]
    async fn test_change_birth_date_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();

        account_repo.add_account(Account::builder(
            account_id.clone(), RegionCode::from_raw("eu"),
            Username::try_new("alex").unwrap(), Email::try_new("alex@test.com").unwrap(),
            ExternalId::from_raw("ext_1")
        ).build());

        let cmd = ChangeBirthDateCommand {
            account_id,
            region_code: RegionCode::from_raw("us"), // Region mismatch
            birth_date: adult_birth_date(),
        };

        let result = use_case.execute(cmd).await;

        // L'entité renvoie Forbidden en cas de mismatch de région
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    // --- CAS 3 : IDEMPOTENCE (AUCUN CHANGEMENT) ---
    #[tokio::test]
    async fn test_change_birth_date_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let date = BirthDate::try_new(Utc.with_ymd_and_hms(1990, 1, 1, 0, 0, 0).unwrap().date_naive()).unwrap();

        let mut account = Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("alex").unwrap(), Email::try_new("alex@test.com").unwrap(),
            ExternalId::from_raw("ext_1")
        ).build();

        // On simule une date déjà présente
        account.change_birth_date(&region, date.clone()).unwrap();
        account_repo.add_account(account);

        let cmd = ChangeBirthDateCommand { account_id: account_id.clone(), region_code: region, birth_date: date };

        // 1. Le Use Case doit renvoyer Ok(false)
        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Ok(false)));

        // 2. La version ne doit pas avoir bougé (toujours 2)
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.version(), 2);

        // 3. Aucun événement dans l'outbox
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    // --- CAS 4 : WORST CASE - COMPTE BLOQUÉ/BANNI ---
    #[tokio::test]
    async fn test_change_birth_date_forbidden_when_banned() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        let mut account = Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("hacker").unwrap(), Email::try_new("h@k.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build();

        account.ban(&region, "Abuse".into()).unwrap();
        account_repo.add_account(account);

        let cmd = ChangeBirthDateCommand {
            account_id, region_code: region,
            birth_date: adult_birth_date(),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    // --- CAS 5 : WORST CASE - COMPTE INEXISTANT ---
    #[tokio::test]
    async fn test_change_birth_date_not_found() {
        let (use_case, _, _) = setup();
        let cmd = ChangeBirthDateCommand {
            account_id: AccountId::new(),
            region_code: RegionCode::from_raw("eu"),
            birth_date: adult_birth_date(),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::NotFound { entity: "Account", .. })));
    }

    // --- CAS 6 : CONCURRENCE EXTRÊME ---
    #[tokio::test]
    async fn test_worst_case_concurrency_exhaustion() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        account_repo.add_account(Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("user").unwrap(), Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        *account_repo.error_to_return.lock().unwrap() = Some(DomainError::ConcurrencyConflict {
            reason: "Always failing".into(),
        });

        let cmd = ChangeBirthDateCommand { account_id, region_code: region, birth_date: adult_birth_date() };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }
}