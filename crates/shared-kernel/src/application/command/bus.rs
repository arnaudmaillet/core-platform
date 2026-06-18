use crate::cache::CacheInvalidator;
use crate::command::{CacheKeyComponent, CommandHandler, IdentifiableCommand};
use crate::core::{Error, Result, with_retry};
use crate::idempotency::IdempotencyRepository;
use async_trait::async_trait;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

#[async_trait]
pub trait AnyCommandHandler: Send + Sync {
    async fn execute_any(
        &self,
        ctx: Arc<dyn Any + Send + Sync>,
        cmd: Arc<dyn Any + Send + Sync>,
    ) -> Result<Arc<dyn Any + Send + Sync>>;
}

pub struct CommandBus {
    handlers: HashMap<TypeId, Arc<dyn AnyCommandHandler>>,
    idempotency: Option<Arc<dyn IdempotencyRepository>>,
    cache_invalidator: Option<Arc<dyn CacheInvalidator>>,
}

impl CommandBus {
    pub fn new(
        idempotency: Option<Arc<dyn IdempotencyRepository>>,
        cache_invalidator: Option<Arc<dyn CacheInvalidator>>,
    ) -> Self {
        Self {
            handlers: HashMap::new(),
            idempotency,
            cache_invalidator,
        }
    }

    pub fn register<TContext, TCommand, THandler>(&mut self, handler: THandler)
    where
        TContext: 'static + Send + Sync + Clone,
        TCommand: IdentifiableCommand + std::fmt::Debug + 'static + Send + Sync + Clone,
        TCommand::Routing: CacheKeyComponent,
        THandler: CommandHandler<Context = TContext, Command = TCommand> + 'static + Send + Sync,
        THandler::Output: 'static + Send + Sync,
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
        TCommand::Routing: CacheKeyComponent,
        TOutput: 'static + Send + Sync + Default + Clone,
    {
        let cmd_id = cmd.command_id();
        let cache_key = cmd.resolve_cache_key();
        let type_id = TypeId::of::<TCommand>();
        let should_handle_idempotency = self.idempotency.is_some() && cmd.is_idempotency_enabled();

        if should_handle_idempotency {
            if let Some(repo) = &self.idempotency {
                if repo.exists(None, &cmd_id).await? {
                    tracing::info!(%cmd_id, "CommandBus: Idempotence triggered. Already processed.");
                    // ATTENTION ICI : Renvoyer Default() peut être dangereux selon le domaine métier.
                    // Idéalement, ton repo d'idempotence devrait stocker et réhydrater le TOutput précédent.
                    return Ok(TOutput::default());
                }
            }
        }

        let handler = self.handlers.get(&type_id).ok_or_else(|| {
            Error::internal(format!(
                "No handler registered for {}",
                std::any::type_name::<TCommand>()
            ))
        })?;

        let result = handler.execute_any(Arc::new(ctx), Arc::new(cmd)).await?;
        if should_handle_idempotency {
            if let Some(repo) = &self.idempotency {
                repo.save(None, &cmd_id).await?;
            }
        }

        if let Some(key) = cache_key {
            if let Some(invalidator) = &self.cache_invalidator {
                let _ = invalidator.invalidate(&key).await;
                tracing::info!(%key, "CommandBus: Read-model cache context successfully evicted");
            }
        }

        let output = result
            .downcast::<TOutput>()
            .map_err(|_| Error::internal("CommandBus: Downcast output failed"))?;

        Ok((*output).clone())
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
    TCommand::Routing: CacheKeyComponent,
    THandler: CommandHandler<Context = TContext, Command = TCommand> + Send + Sync,
    THandler::Output: 'static + Send + Sync,
{
    async fn execute_any(
        &self,
        ctx: Arc<dyn Any + Send + Sync>,
        cmd: Arc<dyn Any + Send + Sync>,
    ) -> Result<Arc<dyn Any + Send + Sync>> {
        use tracing::{Instrument, info_span};

        let concrete_cmd = cmd
            .downcast::<TCommand>()
            .map_err(|_| Error::internal("AnyCommandHandler: Invalid command type"))?;

        let concrete_ctx = ctx
            .downcast::<TContext>()
            .map_err(|_| Error::internal("AnyCommandHandler: Invalid context type"))?;

        let target = concrete_cmd.target();
        let routing_log = concrete_cmd
            .routing()
            .to_key_component()
            .unwrap_or_else(|| "global".to_string());

        let span = info_span!(
            "handle_command",
            command_type = %std::any::type_name::<TCommand>(),
            command_id = %concrete_cmd.command_id(),
            target_id = %target.id,
            routing_strategy = %routing_log
        );

        let config = self.handler.retry_config();

        let result_fut = async {
            tracing::info!("starting command execution");

            let output = with_retry(config, || {
                let c = concrete_ctx.as_ref().clone();
                let command = concrete_cmd.as_ref().clone();
                async move { self.handler.handle(&c, command).await }
            })
            .await;

            match &output {
                Ok(_) => tracing::info!("command executed successfully"),
                Err(e) => tracing::error!(error = %e, "command execution failed"),
            }

            output
        };

        let output = result_fut.instrument(span).await?;
        Ok(Arc::new(output))
    }
}
