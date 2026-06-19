// crates/shared-kernel/src/application/command/identifiable.rs

use crate::command::CacheKeyComponent;
use crate::{command::CommandTarget, core::Identifier};
use uuid::Uuid;

pub trait IdentifiableCommand {
    type Id: Identifier;
    type Routing: CacheKeyComponent;

    fn command_id(&self) -> Uuid;
    fn target(&self) -> &CommandTarget<Self::Id>;
    fn routing(&self) -> Self::Routing;
    fn cache_scope(&self) -> &'static str {
        Self::Id::identifier_scope()
    }
    fn is_idempotency_enabled(&self) -> bool {
        true
    }
    fn resolve_cache_key(&self) -> Option<String> {
        let target = self.target();
        match self.routing().to_key_component() {
            Some(prefix) => Some(format!("{}:{}:{}", self.cache_scope(), prefix, target.id)),
            None => Some(format!("{}:{}", self.cache_scope(), target.id)),
        }
    }
}
