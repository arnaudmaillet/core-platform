use crate::cache::CacheRepository;
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
    cache: Arc<dyn CacheRepository>,
    idempotency: Arc<dyn IdempotencyRepository>,
}

impl CommandBus {
    pub fn new(
        cache: Arc<dyn CacheRepository>,
        idempotency: Arc<dyn IdempotencyRepository>,
    ) -> Self {
        Self {
            handlers: HashMap::new(),
            cache,
            idempotency,
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

        // 1. Coupe-circuit Idempotence
        if self.idempotency.exists(None, &cmd_id).await? {
            tracing::info!(
                command_id = %cmd_id,
                command_type = %std::any::type_name::<TCommand>(),
                "CommandBus: Idempotence technique activée. Commande déjà traitée."
            );
            return Ok(TOutput::default());
        }

        // 2. Résolution du Handler
        let handler = self.handlers.get(&type_id).ok_or_else(|| {
            Error::internal(format!(
                "No handler registered for {}",
                std::any::type_name::<TCommand>()
            ))
        })?;

        // 💡 Utilisation de Arc pour l'effacement de type compatible dyn
        let ctx_arc = Arc::new(ctx);
        let cmd_arc = Arc::new(cmd);

        // 3. Exécution
        let result = handler.execute_any(ctx_arc, cmd_arc).await?;

        // 4. Enregistrement Idempotence
        self.idempotency.save(None, &cmd_id).await?;

        // 5. Invalidation Cache
        if let Some(key) = cache_key {
            let _ = self.cache.delete(&key).await;
            tracing::info!(key = %key, "CommandBus: Cache invalidated");
        }

        // 6. Downcast de l'output depuis l'Arc
        let output = result
            .downcast::<TOutput>()
            .map_err(|_| Error::internal("CommandBus: Downcast output failed"))?;

        // On extrait la valeur de l'Arc en la clonant (TOutput implémente Clone)
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

        // Downcast sécurisé depuis l'Arc
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
