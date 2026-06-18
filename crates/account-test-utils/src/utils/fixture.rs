// crates/account/src/application/fixture.rs

use crate::repositories::{AccountRepositoryStub, GlobalIdentityRegistryStub, OtpRepositoryStub};
use account::AccountServiceBuilder;
use account::context::{AccountCommandCtx, AccountKernelCtx, AccountQueryCtx};
use account::entities::{Account, AccountBuilder};
use account::repositories::{AccountRepository, GlobalIdentityRegistry, OtpRepository};
use account::types::RegistrationIdentifier;
use shared_kernel::command::CommandBus;
use shared_kernel::core::Result;
use shared_kernel::environment::ClusterContext;
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::messaging::OutboxRepository;
use shared_kernel::types::{AccountId, Email, Region};
use shared_kernel_test_utils::repositories::IdempotencyRepositoryStub;
use shared_kernel_test_utils::repositories::OutboxRepositoryStub;
use shared_kernel_test_utils::repositories::{CacheRepositoryStub, TransactionManagerStub};
use std::sync::Arc;

// --- Imports des Use Cases ---

pub struct AccountTestFixture {
    bus: Arc<CommandBus>,
    account_id: AccountId,

    kernel_ctx: AccountKernelCtx,
    command_ctx: AccountCommandCtx,
    query_ctx: AccountQueryCtx,
    cluster_ctx: ClusterContext,

    account_repo: Arc<AccountRepositoryStub>,
    idempotency_repo: Arc<IdempotencyRepositoryStub>,
    outbox_repo: Arc<OutboxRepositoryStub>,
    global_registry: Arc<GlobalIdentityRegistryStub>,
    otp_repo: Arc<OtpRepositoryStub>,
}

impl AccountTestFixture {
    pub fn new() -> Self {
        let account_id = AccountId::generate();

        let tx_manager = Arc::new(TransactionManagerStub);
        let account_repo = Arc::new(AccountRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let idempotency_repo = Arc::new(IdempotencyRepositoryStub::new());
        let cache_repo = Arc::new(CacheRepositoryStub::new());
        let global_registry = Arc::new(GlobalIdentityRegistryStub::new());
        let otp_repo = Arc::new(OtpRepositoryStub::new());

        let cluster_ctx = ClusterContext::default();
        let mut command_bus = CommandBus::new(Some(idempotency_repo.clone()), Some(cache_repo));

        let service = AccountServiceBuilder::new(
            account_repo.clone() as Arc<dyn AccountRepository>,
            outbox_repo.clone() as Arc<dyn OutboxRepository>,
            idempotency_repo.clone() as Arc<dyn IdempotencyRepository>,
            global_registry.clone() as Arc<dyn GlobalIdentityRegistry>,
            otp_repo.clone() as Arc<dyn OtpRepository>,
            tx_manager,
            cluster_ctx,
        );

        let kernel_ctx = service.build_kernel_ctx();
        let command_ctx = kernel_ctx.build_command_ctx(account_id, cluster_ctx.region());
        let query_ctx = kernel_ctx.build_query_ctx(cluster_ctx.region());

        service.register_handlers(&mut command_bus);

        Self {
            bus: Arc::new(command_bus),
            account_id,
            kernel_ctx,
            command_ctx,
            query_ctx,
            account_repo,
            idempotency_repo,
            outbox_repo,
            global_registry,
            otp_repo,
            cluster_ctx,
        }
    }

    pub fn bus(&self) -> &CommandBus {
        &self.bus
    }

    pub fn account_id(&self) -> AccountId {
        self.account_id
    }

    pub fn server_region(&self) -> Region {
        self.cluster_ctx.region()
    }

    pub fn kernel_ctx(&self) -> &AccountKernelCtx {
        &self.kernel_ctx
    }

    pub fn command_ctx(&self) -> &AccountCommandCtx {
        &self.command_ctx
    }

    pub fn query_ctx(&self) -> &AccountQueryCtx {
        &self.query_ctx
    }

    pub fn cluster_ctx(&self) -> &ClusterContext {
        &self.cluster_ctx
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

    pub fn global_registry(&self) -> &GlobalIdentityRegistryStub {
        &self.global_registry
    }

    pub fn otp_repo(&self) -> Arc<OtpRepositoryStub> {
        self.otp_repo.clone()
    }

    pub fn builder(&self) -> Result<AccountBuilder> {
        Ok(Account::builder(
            self.account_id,
            RegistrationIdentifier::from_email(Email::try_new("test@example.com")?),
        ))
    }

    pub fn account_assertions(&self) -> &AccountRepositoryStub {
        &self.account_repo
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
}
