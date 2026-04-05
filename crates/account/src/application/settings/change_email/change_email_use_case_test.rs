#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use crate::domain::account::entities::Account;
    use crate::domain::value_objects::{Email, ExternalId};
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::settings::change_email::{ChangeEmailCommand, ChangeEmailUseCase};
    use crate::domain::repositories::AccountRepositoryStub;

    fn setup() -> (ChangeEmailUseCase, Arc<AccountRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let account_repo = Arc::new(AccountRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = ChangeEmailUseCase::new(account_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, account_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_change_email_success() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let old_email = Email::try_new("old@test.com").unwrap();
        let new_email = Email::try_new("new@test.com").unwrap();
        let region = RegionCode::from_raw("eu");

        // On prépare le compte existant
        account_repo.add_account(Account::builder(
            account_id.clone(), 
            region.clone(),
            old_email.clone(),
            ExternalId::from_raw("ext_1")
        ).build());

        let cmd = ChangeEmailCommand {
            account_id: account_id.clone(),
            region_code: region,
            new_email: new_email.clone(),
        };

        // 1. Act : On récupère l'Account retourné par le Use Case
        let result = use_case.execute(cmd).await;
        
        // On vérifie que c'est un succès
        assert!(result.is_ok());
        let updated_account = result.unwrap();

        // 2. Assert : Vérifier l'objet retourné (Mémoire)
        assert_eq!(updated_account.email(), &new_email);
        assert!(!updated_account.is_email_verified());

        // 3. Assert : Vérifier la persistence (Mock Repo)
        let saved = account_repo.accounts_map.lock().unwrap()
            .get(&account_id)
            .cloned()
            .unwrap();
            
        assert_eq!(saved.email(), &new_email);
        assert_eq!(saved.version(), 2);
        
        // 4. Assert : Vérifier l'Outbox (Événements)
        let events = outbox_repo.saved_events.lock().unwrap();
        assert_eq!(events.len(), 1);
    }

    #[tokio::test]
    async fn test_change_email_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let email = Email::try_new("same@test.com").unwrap();
        let region = RegionCode::from_raw("eu");

        // On insère un compte avec la version 1
        let initial_account = Account::builder(
            account_id.clone(), 
            region.clone(),
            email.clone(),
            ExternalId::from_raw("ext_1")
        ).build();
        
        account_repo.add_account(initial_account.clone());

        let cmd = ChangeEmailCommand {
            account_id: account_id.clone(),
            region_code: region,
            new_email: email.clone()
        };

        // 1. Act : L'exécution doit réussir mais ne rien modifier
        let result = use_case.execute(cmd).await;
        
        assert!(result.is_ok());
        let returned_account = result.unwrap();

        // 2. Assert : L'objet retourné doit être identique à l'initial
        assert_eq!(returned_account.email(), &email);
        assert_eq!(returned_account.version(), 1);

        // 3. Assert : Rien ne doit avoir été persisté (pas d'appel à save inutile)
        let saved_in_db = account_repo.accounts_map.lock().unwrap()
            .get(&account_id).cloned().unwrap();
        assert_eq!(saved_in_db.version(), 1);

        // 4. Assert : Crucial - Aucun événement ne doit être produit
        let events = outbox_repo.saved_events.lock().unwrap();
        assert_eq!(events.len(), 0);
    }

    #[tokio::test]
    async fn test_change_email_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let actual_region = RegionCode::from_raw("eu");
        let wrong_region = RegionCode::from_raw("us");

        account_repo.add_account(Account::builder(
            account_id.clone(), actual_region,
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext_1")
        ).build());

        let cmd = ChangeEmailCommand {
            account_id,
            region_code: wrong_region,
            new_email: Email::try_new("new@test.com").unwrap(),
        };

        let result = use_case.execute(cmd).await;

        // Sécurité : mismatch de région = Forbidden
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_change_email_forbidden_when_restricted() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        let mut account = Account::builder(
            account_id.clone(), region.clone(),
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext_1")
        ).build();

        // Un banni ne change pas son email
        account.ban(&region, "Violation".into()).unwrap();
        account_repo.add_account(account);

        let cmd = ChangeEmailCommand {
            account_id,
            region_code: region,
            new_email: Email::try_new("new@b.com").unwrap(),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_worst_case_concurrency_conflict() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        account_repo.add_account(Account::builder(
            account_id.clone(), region.clone(),
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext_1")
        ).build());

        // Simulation d'un conflit de version (Optimistic Lock)
        *account_repo.error_to_return.lock().unwrap() = Some(DomainError::ConcurrencyConflict {
            reason: "Version mismatch".into(),
        });

        let cmd = ChangeEmailCommand {
            account_id,
            region_code: region,
            new_email: Email::try_new("b@c.com").unwrap()
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }
}