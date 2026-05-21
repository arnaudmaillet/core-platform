// crates/shared-kernel/src/application/command/identifiable.rs

use uuid::Uuid;

pub trait IdentifiableCommand {
    fn command_id(&self) -> Uuid;
    fn aggregate_id(&self) -> String;
    fn region(&self) -> String;
    fn cache_key(&self) -> Option<String> {
        None
    }
}
