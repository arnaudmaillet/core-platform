use crate::domain::utils::with_retry;
use crate::errors::{DomainError, Result};
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
    // On stocke les handlers avec le type AnyCommandHandler "effacé"
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
        TCommand: 'static + Send + Sync + Clone,
        THandler: crate::application::CommandHandler<Context = TContext, Command = TCommand>
            + 'static
            + Send
            + Sync,
        THandler::Output: 'static + Send,
    {
        // Création explicite du wrapper
        let wrapper = HandlerWrapper {
            handler,
            _phantom: PhantomData::<(TContext, TCommand)>, // On utilise un tuple simple
        };

        // Conversion explicite en Arc<dyn AnyCommandHandler>
        // C'est cette ligne qui fait le "type erasure"
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
        TCommand: 'static + Send + Sync + Clone,
        TOutput: 'static + Send,
    {
        let type_id = TypeId::of::<TCommand>();
        let handler = self.handlers.get(&type_id).ok_or_else(|| {
            DomainError::Internal(format!(
                "No handler registered for {}",
                std::any::type_name::<TCommand>()
            ))
        })?;

        let ctx_box: Box<dyn Any + Send + Sync> = Box::new(ctx);
        let cmd_box: Box<dyn Any + Send> = Box::new(cmd);

        let result = handler.execute_any(ctx_box, cmd_box).await?;

        let output = result
            .downcast::<TOutput>()
            .map_err(|_| DomainError::Internal("CommandBus: Downcast output failed".to_string()))?;

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
    TCommand: 'static + Send + Sync + Clone,
    THandler:
        crate::application::CommandHandler<Context = TContext, Command = TCommand> + Send + Sync,
    THandler::Output: 'static + Send,
{
    async fn execute_any(
        &self,
        ctx: Box<dyn Any + Send + Sync>,
        cmd: Box<dyn Any + Send>,
    ) -> Result<Box<dyn Any + Send>> {
        // Cast des types Boxés vers les types concrets
        let concrete_cmd = cmd
            .downcast::<TCommand>()
            .map_err(|_| DomainError::Internal("AnyCommandHandler: Invalid command type".into()))?;

        let concrete_ctx = ctx
            .downcast::<TContext>()
            .map_err(|_| DomainError::Internal("AnyCommandHandler: Invalid context type".into()))?;

        let config = self.handler.retry_config();

        // Application de la logique de retry
        let output = with_retry(config, || {
            let c = concrete_ctx.clone();
            let command = concrete_cmd.clone();
            async move { self.handler.handle(&c, *command).await }
        })
        .await?;

        Ok(Box::new(output))
    }
}
