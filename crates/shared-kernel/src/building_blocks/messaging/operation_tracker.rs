// crates/shared-kernel/src/core/identity/operation_tracker.rs

use crate::{
    core::{ManagedEntity, Result},
    messaging::Event,
};

pub trait OperationTracker: ManagedEntity {
    fn track_change<F, E>(&mut self, action: F, event_factory: E) -> Result<bool>
    where
        Self: Sized,
        F: FnOnce(&mut Self) -> Result<bool>,
        E: FnOnce(&Self) -> Box<dyn Event>,
    {
        // 1. Exécuter l'action métier
        if action(self)? {
            self.lifecycle_mut().record_change();

            let event = event_factory(self);
            self.push_event(event);

            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl<T: ManagedEntity> OperationTracker for T {}
