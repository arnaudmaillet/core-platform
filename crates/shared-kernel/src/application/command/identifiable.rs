// crates/shared-kernel/src/application/command/identifiable.rs

use crate::{command::CommandTarget, core::Identifier};
use uuid::Uuid;

pub trait IdentifiableCommand {
    type Id: Identifier;
    fn command_id(&self) -> Uuid;
    fn target(&self) -> &CommandTarget<Self::Id>;

    fn cache_scope(&self) -> &'static str {
        Self::Id::identifier_scope()
    }

    fn cache_enabled(&self) -> bool {
        true
    }

    fn resolve_cache_key(&self) -> Option<String> {
        if !self.cache_enabled() {
            return None;
        }

        let target = self.target();
        Some(format!(
            "{}:{}:{}",
            self.cache_scope(),
            target.region.as_str(),
            target.id
        ))
    }
}
