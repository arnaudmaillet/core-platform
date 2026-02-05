#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::domain::entities::Account;
    use crate::domain::value_objects::{Email, ExternalId, Locale};
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, Username, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::update_locale::{UpdateLocaleCommand, UpdateLocaleUseCase};
    use crate::domain::repositories::AccountRepositoryStub;

    fn setup() -> (UpdateLocaleUseCase, Arc<AccountRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let account_repo = Arc::new(AccountRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = UpdateLocaleUseCase::new(account_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, account_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_update_locale_success() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // ✅ Utilisation du Builder pour créer l'état initial (fr par défaut ou non spécifié)
        let account = Account::builder(
            account_id.clone(),
            region.clone(),
            Username::try_new("john_doe").unwrap(),
            Email::try_new("john@example.com").unwrap(),
            ExternalId::from_raw("ext_123")
        )
            .with_locale(Locale::from_raw("fr"))
            .build();

        account_repo.add_account(account);

        let new_locale = Locale::from_raw("en");
        let cmd = UpdateLocaleCommand {
            account_id: account_id.clone(),
            region_code: region,
            locale: new_locale.clone(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.locale(), &new_locale);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_update_locale_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let current_locale = Locale::from_raw("de");

        // ✅ Arrange : déjà en allemand
        let mut account = Account::builder(
            account_id.clone(),
            region.clone(),
            Username::try_new("hans").unwrap(),
            Email::try_new("hans@test.de").unwrap(),
            ExternalId::from_raw("ext_456")
        )
            .with_locale(current_locale.clone())
            .build();

        account.pull_events(); // Nettoyage
        account_repo.add_account(account);

        let cmd = UpdateLocaleCommand {
            account_id,
            region_code: region,
            locale: current_locale,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        // L'entité détecte qu'il n'y a pas de changement -> pas d'event -> pas de save transactionnel
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_update_locale_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();

        // Compte en EU
        account_repo.add_account(Account::builder(
            account_id.clone(),
            RegionCode::from_raw("eu"),
            Username::try_new("traveler").unwrap(),
            Email::try_new("t@t.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = UpdateLocaleCommand {
            account_id,
            region_code: RegionCode::from_raw("us"), // 👈 Mismatch
            locale: Locale::from_raw("en"),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::Validation { field, .. }) if field == "region_code"));
    }
}