// crates/shared-kernel/src/domain/events/traits.rs

use crate::messaging::Event;

/// Capacité d'un objet à produire des événements métier.
pub trait EventEmitter {
    fn push_event(&mut self, event: Box<dyn Event>);
    fn pull_events(&mut self) -> Vec<Box<dyn Event>>;
}
