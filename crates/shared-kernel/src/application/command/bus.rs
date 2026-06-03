use crate::cache::CacheRepository;
use crate::command::{CommandHandler, IdentifiableCommand};
use crate::core::{Error, ErrorCode, Result, with_retry};
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

pub struct CommandBus {
    handlers: HashMap<TypeId, Arc<dyn AnyCommandHandler>>,
    cache: Arc<dyn CacheRepository>,
}

impl CommandBus {
    pub fn new(cache: Arc<dyn CacheRepository>) -> Self {
        Self {
            handlers: HashMap::new(),
            cache,
        }
    }

    pub fn register<TContext, TCommand, THandler>(&mut self, handler: THandler)
    where
        TContext: 'static + Send + Sync + Clone,
        TCommand: IdentifiableCommand + std::fmt::Debug + 'static + Send + Sync + Clone,
        THandler: CommandHandler<Context = TContext, Command = TCommand> + 'static + Send + Sync,
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
        TOutput: 'static + Send + Default,
    {
        let cache_key = cmd.resolve_cache_key();

        let type_id = TypeId::of::<TCommand>();

        let handler = self.handlers.get(&type_id).ok_or_else(|| {
            Error::internal(format!(
                "No handler registered for {}",
                std::any::type_name::<TCommand>()
            ))
        })?;

        let ctx_box = Box::new(ctx);
        let cmd_box = Box::new(cmd);

        let result = handler.execute_any(ctx_box, cmd_box).await;

        let final_result = match result {
            Err(e) if e.code == ErrorCode::AlreadyExists && e.message.contains("Command") => {
                return Ok(TOutput::default());
            }
            res => res?,
        };

        if let Some(key) = cache_key {
            let _ = self.cache.delete(&key).await;
            tracing::info!(key = %key, "CommandBus: Cache invalidated");
        }

        let output = final_result
            .downcast::<TOutput>()
            .map_err(|_| Error::internal("CommandBus: Downcast output failed"))?;

        Ok(*output)
    }
}

struct HandlerWrapper<THandler, TContext, TCommand> {
    handler: THandler,
    _phantom: PhantomData<(TContext, TCommand)>,
}

#[async_trait]
impl<THandler, TContext, TCommand> AnyCommandHandler
    for HandlerWrapper<THandler, TContext, TCommand>
where
    TContext: 'static + Send + Sync + Clone,
    TCommand: IdentifiableCommand + std::fmt::Debug + 'static + Send + Sync + Clone,
    THandler: CommandHandler<Context = TContext, Command = TCommand> + Send + Sync,
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

        let target = concrete_cmd.target();
        let span = info_span!(
            "handle_command",
            command_type = %std::any::type_name::<TCommand>(),
            command_id = %concrete_cmd.command_id(),
            target_id = %target.id,
            region = %target.region.as_str()
        );

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

        let output = result_fut.instrument(span).await?;

        Ok(Box::new(output))
    }
}
