// crates/shared-kernel/src/application/command_bus.rs

use crate::{
    application::CommandHandler,
    domain::utils::{RetryConfig, with_retry},
    errors::Result,
};

#[derive(Default)]
pub struct CommandBus;

impl CommandBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn execute<TContext, TCommand, THandler>(
        &self,
        ctx: &TContext,
        cmd: TCommand,
        handler: THandler,
    ) -> Result<THandler::Output>
    where
        THandler: CommandHandler<Context = TContext, Command = TCommand>,
        TCommand: Clone + Send + Sync,
    {
        with_retry(RetryConfig::default(), || async {
            handler.handle(ctx, cmd.clone()).await
        })
        .await
    }
}
