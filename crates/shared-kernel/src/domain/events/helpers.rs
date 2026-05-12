use crate::domain::entities::Versioned;
use crate::domain::events::{DomainEvent, traits::EventEmitter};
use crate::core::Result;

pub trait OperationTracker: Versioned + EventEmitter {
    /// Coordonne : Action métier -> Versioning -> Événement
    fn track_change<F, E>(&mut self, action: F, event_factory: E) -> Result<bool>
    where
        Self: Sized,
        F: FnOnce(&mut Self) -> Result<bool>,
        E: FnOnce(&Self) -> Box<dyn DomainEvent>,
    {
        // 1. Exécuter l'action (qui peut échouer ou ne rien changer)
        if action(self)? {
            // 2. Marquer le changement technique (Version + Date)
            self.record_change();

            // 3. Produire et stocker l'événement
            let event = event_factory(self);
            self.push_event(event);

            Ok(true)
        } else {
            Ok(false)
        }
    }
}

// Implémentation automatique
impl<T: Versioned + EventEmitter> OperationTracker for T {}
