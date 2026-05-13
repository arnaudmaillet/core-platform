use crate::application::IdentifiableCommand;
use crate::core::{Error, Result, with_retry};
use async_trait::async_trait;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

#[async_trait]
pub trait AnyCommandHandler: Send + Sync {
    async fn execute_any(
        &self,
        ctx: Box<dyn Any + Send + Sync>,
        cmd: Box<dyn Any + Send>,
    ) -> Result<Box<dyn Any + Send>>;
}

#[derive(Default)]
pub struct CommandBus {
    handlers: HashMap<TypeId, Arc<dyn AnyCommandHandler>>,
}

impl CommandBus {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn register<TContext, TCommand, THandler>(&mut self, handler: THandler)
    where
        TContext: 'static + Send + Sync + Clone,
        TCommand: IdentifiableCommand + std::fmt::Debug + 'static + Send + Sync + Clone,
        THandler: crate::application::CommandHandler<Context = TContext, Command = TCommand>
            + 'static
            + Send
            + Sync,
        THandler::Output: 'static + Send,
    {
        let wrapper = HandlerWrapper {
            handler,
            _phantom: PhantomData::<(TContext, TCommand)>,
        };

        let arc_handler: Arc<dyn AnyCommandHandler> = Arc::new(wrapper);

        self.handlers.insert(TypeId::of::<TCommand>(), arc_handler);
    }

    pub async fn execute<TContext, TCommand, TOutput>(
        &self,
        ctx: TContext,
        cmd: TCommand,
    ) -> Result<TOutput>
    where
        TContext: 'static + Send + Sync + Clone,
        TCommand: IdentifiableCommand + std::fmt::Debug + 'static + Send + Sync + Clone,
        TOutput: 'static + Send,
    {
        let type_id = TypeId::of::<TCommand>();
        let handler = self.handlers.get(&type_id).ok_or_else(|| {
            Error::internal(format!(
                "No handler registered for {}",
                std::any::type_name::<TCommand>()
            ))
        })?;

        let ctx_box: Box<dyn Any + Send + Sync> = Box::new(ctx);
        let cmd_box: Box<dyn Any + Send> = Box::new(cmd);

        let result = handler.execute_any(ctx_box, cmd_box).await?;

        let output = result
            .downcast::<TOutput>()
            .map_err(|_| Error::internal("CommandBus: Downcast output failed"))?;

        Ok(*output)
    }
}

// Correction du PhantomData : on n'utilise plus la signature de fonction fn()
struct HandlerWrapper<THandler, TContext, TCommand> {
    handler: THandler,
    _phantom: PhantomData<(TContext, TCommand)>,
}

#[async_trait]
impl<THandler, TContext, TCommand> AnyCommandHandler
    for HandlerWrapper<THandler, TContext, TCommand>
where
    TContext: 'static + Send + Sync + Clone,
    TCommand: IdentifiableCommand + std::fmt::Debug + 'static + Send + Sync + Clone, // Ajout des bounds ici
    THandler:
        crate::application::CommandHandler<Context = TContext, Command = TCommand> + Send + Sync,
    THandler::Output: 'static + Send,
{
    async fn execute_any(
        &self,
        ctx: Box<dyn Any + Send + Sync>,
        cmd: Box<dyn Any + Send>,
    ) -> Result<Box<dyn Any + Send>> {
        use tracing::{Instrument, info_span};

        let concrete_cmd = cmd
            .downcast::<TCommand>()
            .map_err(|_| Error::internal("AnyCommandHandler: Invalid command type"))?;

        let concrete_ctx = ctx
            .downcast::<TContext>()
            .map_err(|_| Error::internal("AnyCommandHandler: Invalid context type"))?;

        // 1. Préparation des métadonnées de logging (venant du trait IdentifiableCommand)
        let span = info_span!(
            "handle_command",
            command_type = %std::any::type_name::<TCommand>(),
            command_id = %concrete_cmd.command_id(),
            profile_id = %concrete_cmd.profile_id(),
            region = %concrete_cmd.region()
        );

        // 2. Exécution enveloppée dans la span et le retry
        let config = self.handler.retry_config();

        let result_fut = async {
            tracing::info!("starting command execution");

            let output = with_retry(config, || {
                let c = concrete_ctx.clone();
                let command = concrete_cmd.clone();
                async move { self.handler.handle(&c, *command).await }
            })
            .await;

            match &output {
                Ok(_) => tracing::info!("command executed successfully"),
                Err(e) => tracing::error!(error = %e, "command execution failed"),
            }

            output
        };

        // On instrumente le futur et on attend le résultat
        let output = result_fut.instrument(span).await?;

        Ok(Box::new(output))
    }
}
