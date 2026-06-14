use std::sync::Arc;

use crate::{
    commands::{
        ActivateCommand, ActivateHandler, AddPushTokenCommand, AddPushTokenHandler, BanCommand,
        BanHandler, ChangeBetaTierCommand, ChangeBetaTierHandler, ChangeBirthDateCommand,
        ChangeBirthDateHandler, ChangeEmailCommand, ChangeEmailHandler, ChangePhoneCommand,
        ChangePhoneNumberHandler, ChangeRoleCommand, ChangeRoleHandler, DeactivateCommand,
        DeactivateHandler, DecreaseTrustScoreCommand, DecreaseTrustScoreHandler,
        IncreaseTrustScoreCommand, IncreaseTrustScoreHandler, LiftShadowbanCommand,
        LiftShadowbanHandler, LinkSubIdentityCommand, LinkSubIdentityHandler, RegisterCommand,
        RegisterHandler, RemovePushTokenCommand, RemovePushTokenHandler, ShadowbanCommand,
        ShadowbanHandler, SuspendCommand, SuspendHandler, UnbanCommand, UnbanHandler,
        UnsuspendCommand, UnsuspendHandler, UpdateLocaleCommand, UpdateLocaleHandler,
        UpdatePreferencesCommand, UpdatePreferencesHandler, UpdateTimezoneCommand,
        UpdateTimezoneHandler, VerifyEmailCommand, VerifyEmailHandler, VerifyPhoneCommand,
        VerifyPhoneHandler,
    },
    context::{AccountCommandCtx, AccountKernelCtx},
    repositories::{AccountRepository, GlobalIdentityRegistry, OtpRepository},
};
use shared_kernel::{
    command::CommandBus, core::TransactionManager, environment::ClusterContext,
    idempotency::IdempotencyRepository, messaging::OutboxRepository,
};

pub struct AccountServiceBuilder {
    account_repo: Arc<dyn AccountRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
    global_registry: Arc<dyn GlobalIdentityRegistry>,
    otp_repo: Arc<dyn OtpRepository>,
    tx_manager: Arc<dyn TransactionManager>,
    cluster_ctx: ClusterContext,
}

impl AccountServiceBuilder {
    pub fn new(
        account_repo: Arc<dyn AccountRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
        global_registry: Arc<dyn GlobalIdentityRegistry>,
        otp_repo: Arc<dyn OtpRepository>,
        tx_manager: Arc<dyn TransactionManager>,
        cluster_ctx: ClusterContext,
    ) -> Self {
        Self {
            account_repo,
            outbox_repo,
            idempotency_repo,
            global_registry,
            otp_repo,
            tx_manager,
            cluster_ctx,
        }
    }

    pub fn build_kernel_ctx(&self) -> AccountKernelCtx {
        AccountKernelCtx::new(
            self.tx_manager.clone(),
            self.account_repo.clone(),
            self.outbox_repo.clone(),
            self.idempotency_repo.clone(),
            self.global_registry.clone(),
            self.cluster_ctx,
        )
    }

    pub fn register_handlers(&self, bus: &mut CommandBus) {
        bus.register::<AccountCommandCtx, RegisterCommand, RegisterHandler>(RegisterHandler::new());
        bus.register::<AccountCommandCtx, LinkSubIdentityCommand, LinkSubIdentityHandler>(
            LinkSubIdentityHandler::new(),
        );
        bus.register::<AccountCommandCtx, VerifyEmailCommand, VerifyEmailHandler>(
            VerifyEmailHandler::new(self.otp_repo.clone()),
        );
        bus.register::<AccountCommandCtx, VerifyPhoneCommand, VerifyPhoneHandler>(
            VerifyPhoneHandler::new(self.otp_repo.clone()),
        );
        bus.register::<AccountCommandCtx, ActivateCommand, ActivateHandler>(ActivateHandler::new());
        bus.register::<AccountCommandCtx, DeactivateCommand, DeactivateHandler>(
            DeactivateHandler::new(),
        );
        bus.register::<AccountCommandCtx, ChangeRoleCommand, ChangeRoleHandler>(
            ChangeRoleHandler::new(),
        );
        bus.register::<AccountCommandCtx, SuspendCommand, SuspendHandler>(SuspendHandler::new());
        bus.register::<AccountCommandCtx, UnsuspendCommand, UnsuspendHandler>(
            UnsuspendHandler::new(),
        );
        bus.register::<AccountCommandCtx, BanCommand, BanHandler>(BanHandler::new());
        bus.register::<AccountCommandCtx, UnbanCommand, UnbanHandler>(UnbanHandler::new());
        bus.register::<AccountCommandCtx, ShadowbanCommand, ShadowbanHandler>(
            ShadowbanHandler::new(),
        );
        bus.register::<AccountCommandCtx, LiftShadowbanCommand, LiftShadowbanHandler>(
            LiftShadowbanHandler::new(),
        );
        bus.register::<AccountCommandCtx, IncreaseTrustScoreCommand, IncreaseTrustScoreHandler>(
            IncreaseTrustScoreHandler::new(),
        );
        bus.register::<AccountCommandCtx, DecreaseTrustScoreCommand, DecreaseTrustScoreHandler>(
            DecreaseTrustScoreHandler::new(),
        );
        bus.register::<AccountCommandCtx, AddPushTokenCommand, AddPushTokenHandler>(
            AddPushTokenHandler::new(),
        );
        bus.register::<AccountCommandCtx, RemovePushTokenCommand, RemovePushTokenHandler>(
            RemovePushTokenHandler::new(),
        );
        bus.register::<AccountCommandCtx, ChangeEmailCommand, ChangeEmailHandler>(
            ChangeEmailHandler::new(),
        );
        bus.register::<AccountCommandCtx, ChangePhoneCommand, ChangePhoneNumberHandler>(
            ChangePhoneNumberHandler::new(),
        );
        bus.register::<AccountCommandCtx, ChangeBirthDateCommand, ChangeBirthDateHandler>(
            ChangeBirthDateHandler::new(),
        );
        bus.register::<AccountCommandCtx, UpdateLocaleCommand, UpdateLocaleHandler>(
            UpdateLocaleHandler::new(),
        );
        bus.register::<AccountCommandCtx, UpdateTimezoneCommand, UpdateTimezoneHandler>(
            UpdateTimezoneHandler::new(),
        );
        bus.register::<AccountCommandCtx, UpdatePreferencesCommand, UpdatePreferencesHandler>(
            UpdatePreferencesHandler::new(),
        );
        bus.register::<AccountCommandCtx, ChangeBetaTierCommand, ChangeBetaTierHandler>(
            ChangeBetaTierHandler::new(),
        );
    }
}
