// crates/account/src/application/test_utils.rs

use shared_kernel::application::{BaseAppContext, CommandBus};
use shared_kernel::domain::repositories::{
    CacheRepositoryStub, IdempotencyRepositoryStub, OutboxRepositoryStub,
};
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::errors::Result;
use std::sync::Arc;

use crate::application::context::{AccountAppContext, AccountContext};
use crate::domain::account::builders::AccountBuilder;
use crate::domain::account::entities::Account;
use crate::domain::repositories::AccountRepositoryStub;
use crate::domain::value_objects::{RegistrationIdentifier};

pub struct TestFixture {
    bus: CommandBus,
    app_ctx: AccountAppContext,
    account_ctx: AccountContext,
    account_repo: Arc<AccountRepositoryStub>,
    idempotency_repo: Arc<IdempotencyRepositoryStub>,
    outbox_repo: Arc<OutboxRepositoryStub>,
}

impl TestFixture {
    pub fn new() -> Self {
        // 1. Initialisation des Stubs de bas niveau
        let account_repo = Arc::new(AccountRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let idempotency_repo = Arc::new(IdempotencyRepositoryStub::new());
        let cache = Arc::new(CacheRepositoryStub::new());

        // 2. Création du BaseAppContext (Simule shared-kernel)
        // On passe None pour le pool car nos Stubs n'en ont pas besoin
        let base_ctx = BaseAppContext::new(None, cache);

        // 3. Création de l'AccountAppContext (Infrastructure du module)
        let app_ctx = AccountAppContext::new(
            base_ctx,
            account_repo.clone(),
            outbox_repo.clone(),
            idempotency_repo.clone(),
        );

        // 4. Création du contexte Scoped par défaut pour les tests
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let account_ctx = AccountContext::new(app_ctx.clone(), account_id, region);

        Self {
            bus: CommandBus::new(),
            app_ctx,
            account_ctx,
            account_repo,
            idempotency_repo,
            outbox_repo,
        }
    }

    // --- Accesseurs pour les tests ---

    pub fn bus(&self) -> &CommandBus {
        &self.bus
    }

    /// Pour les créations (Register)
    pub fn app_ctx(&self) -> &AccountAppContext {
        &self.app_ctx
    }

    /// Pour les modifications (Settings, etc.)
    pub fn account_ctx(&self) -> &AccountContext {
        &self.account_ctx
    }

    pub fn account_id(&self) -> AccountId {
        self.account_ctx.account_id().clone()
    }

    pub fn region(&self) -> RegionCode {
        self.account_ctx.region().clone()
    }

    pub fn account_repo(&self) -> &AccountRepositoryStub {
        &self.account_repo
    }

    pub fn idempotency_repo(&self) -> &IdempotencyRepositoryStub {
        &self.idempotency_repo
    }

    pub fn outbox_repo(&self) -> &OutboxRepositoryStub {
        &self.outbox_repo
    }

    pub fn outbox_events(&self) -> Vec<String> {
        self.outbox_repo.event_names()
    }

    pub fn account_builder(&self) -> Result<AccountBuilder> {
        self.account_builder_for(self.region())
    }

    /// Cas spécifique : permet d'injecter une région différente
    pub fn account_builder_for(&self, region: RegionCode) -> Result<AccountBuilder> {
        Ok(Account::builder(
            self.account_id(),
            region,
            RegistrationIdentifier::try_from_email("test@example.com")?,
        ))
    }

    pub fn assert_outbox(&self, expected_count: usize, expected_event: Option<&str>) {
        assert_eq!(
            self.outbox_repo().count(),
            expected_count,
            "Nombre d'événements incorrect"
        );
        if let Some(event_name) = expected_event {
            assert!(
                self.outbox_events().contains(&event_name.to_string()),
                "L'événement {} est manquant dans l'outbox",
                event_name
            );
        }
    }

    /// Vérifie le compte "par défaut" de la fixture
    pub async fn assert_account<F>(&self, check: F) -> Result<()>
    where
        F: FnOnce(&Account),
    {
        self.assert_account_by_id(&self.account_id(), check).await
    }

    /// NOUVEAU : Indispensable pour le Register car l'ID est généré dynamiquement
    pub async fn assert_account_by_id<F>(&self, id: &AccountId, check: F) -> Result<()>
    where
        F: FnOnce(&Account),
    {
        let saved = self
            .account_repo
            .find_direct(id) // Utilise la méthode de ton Stub
            .expect("Le compte devrait exister dans le repository");

        check(&saved);
        Ok(())
    }

    pub async fn assert_account_exists(&self, id: &AccountId) -> Result<()> {
        let account = self.account_repo().find_direct(id);

        assert!(
            account.is_some(),
            "Le compte avec l'ID {} devrait exister dans le repository",
            id
        );
        Ok(())
    }

    pub fn assert_outbox_contains(&self, event_name: &str) {
        assert!(
            self.outbox_events().contains(&event_name.to_string()),
            "L'événement {} est manquant dans l'outbox. Événements présents : {:?}",
            event_name,
            self.outbox_events()
        );
    }
}
