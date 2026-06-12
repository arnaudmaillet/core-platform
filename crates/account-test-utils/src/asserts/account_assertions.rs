// crates/account/src/application/assertions.rs

use crate::repositories::AccountRepositoryStub;
use account::entities::Account;
use async_trait::async_trait;
use shared_kernel::messaging::Event;
use shared_kernel::types::AccountId;

#[async_trait]
pub trait AccountRepositoryAsserts {
    async fn assert_account_state<F>(&self, account_id: AccountId, check: F)
    where
        F: FnOnce(&Account) + Send;

    async fn assert_captured_event_for<E, F>(&self, account_id: AccountId, check: F)
    where
        E: Event + 'static,
        F: FnOnce(&E) + Send;

    async fn assert_no_events_for(&self, account_id: AccountId);
}

#[async_trait]
impl AccountRepositoryAsserts for AccountRepositoryStub {
    async fn assert_account_state<F>(&self, account_id: AccountId, check: F)
    where
        F: FnOnce(&Account) + Send,
    {
        let saved = self
            .find_direct(account_id)
            .expect("Assertion Failed: Account expected to exist in repository stub");
        check(&saved);
    }

    async fn assert_captured_event_for<E, F>(&self, account_id: AccountId, check: F)
    where
        E: Event + 'static,
        F: FnOnce(&E) + Send,
    {
        let events = self.get_captured_events(account_id).await;
        assert_eq!(
            events.len(),
            1,
            "Assertion Failed: Expected exactly 1 domain event for account {:?}",
            account_id
        );

        let generic_event = events[0].as_any();
        let concrete_event = generic_event
            .downcast_ref::<E>()
            .expect("Assertion Failed: Captured event could not be downcast to the expected concrete Event type");

        check(concrete_event);
    }

    async fn assert_no_events_for(&self, account_id: AccountId) {
        let events = self.get_captured_events(account_id).await;
        assert!(
            events.is_empty(),
            "Assertion Failed: Expected 0 events, but captured: {:?}",
            events
        );
    }
}
