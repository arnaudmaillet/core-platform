#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::domain::entities::Account;
    use crate::domain::value_objects::{Email, ExternalId};
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, Username, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::link_external_identity::{LinkExternalIdentityCommand, LinkExternalIdentityUseCase};
    use crate::domain::repositories::AccountRepositoryStub;

    fn setup() -> (LinkExternalIdentityUseCase, Arc<AccountRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let account_repo = Arc::new(AccountRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = LinkExternalIdentityUseCase::new(account_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, account_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_link_external_identity_success() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // ✅ Initialisation avec un ID vide pour autoriser le premier linkage
        account_repo.add_account(Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("alex").unwrap(),
            Email::try_new("alex@test.com").unwrap(),
            ExternalId::from_raw("")
        ).build());

        let new_ext = ExternalId::from_raw("google_123");
        let cmd = LinkExternalIdentityCommand {
            internal_account_id: account_id.clone(),
            region_code: region,
            external_id: new_ext.clone(),
        };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        let saved = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.external_id(), &new_ext);
    }

    #[tokio::test]
    async fn test_link_external_identity_conflict_already_taken() {
        let (use_case, account_repo, _) = setup();
        let region = RegionCode::from_raw("eu");

        // Arrange: Alice possède déjà l'ID externe "google_123"
        let alice_id = AccountId::new();
        let shared_ext = ExternalId::from_raw("google_123");

        account_repo.add_account(Account::builder(
            alice_id.clone(), region.clone(),
            Username::try_new("alice").unwrap(), Email::try_new("alice@test.com").unwrap(),
            shared_ext.clone()
        ).build());

        // Bob essaie de lier le même ID externe
        let bob_id = AccountId::new();
        account_repo.add_account(Account::builder(
            bob_id.clone(), region.clone(),
            Username::try_new("bob").unwrap(), Email::try_new("bob@test.com").unwrap(),
            ExternalId::from_raw("bob_ext")
        ).build());

        let cmd = LinkExternalIdentityCommand {
            internal_account_id: bob_id,
            region_code: region,
            external_id: shared_ext,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::AlreadyExists { field, .. }) if field == "external_id"));
    }

    #[tokio::test]
    async fn test_link_external_identity_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let ext_id = ExternalId::from_raw("steam_456");

        // Arrange: Le compte a déjà cet ID externe
        account_repo.add_account(Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("gamer").unwrap(), Email::try_new("g@m.com").unwrap(),
            ext_id.clone()
        ).build());

        let cmd = LinkExternalIdentityCommand {
            internal_account_id: account_id,
            region_code: region,
            external_id: ext_id,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0, "Aucun changement si déjà lié");
    }

    #[tokio::test]
    async fn test_link_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();

        account_repo.add_account(Account::builder(
            account_id.clone(), RegionCode::from_raw("eu"),
            Username::try_new("user").unwrap(), Email::try_new("u@t.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = LinkExternalIdentityCommand {
            internal_account_id: account_id,
            region_code: RegionCode::from_raw("us"), // Region mismatch
            external_id: ExternalId::from_raw("new_ext"),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::Validation { field, .. }) if field == "region_code"));
    }
}