use async_trait::async_trait;
use profile::entities::Profile;
use profile::events::ProfileEvent;
use shared_kernel::types::ProfileId;
use crate::repositories::ProfileRepositoryStub;

#[async_trait]
pub trait ProfileRepositoryAsserts {
    async fn assert_profile_state<F>(&self, profile_id: ProfileId, check: F) where F: FnOnce(&Profile) + Send;
    async fn assert_captured_event_for<F>(&self, profile_id: ProfileId, check: F) where F: FnOnce(&ProfileEvent) + Send;
    async fn assert_no_events_for(&self, profile_id: ProfileId);
}

#[async_trait]
impl ProfileRepositoryAsserts for ProfileRepositoryStub {
    async fn assert_profile_state<F>(&self, profile_id: ProfileId, check: F) 
    where 
        F: FnOnce(&Profile) + Send 
    {
        let saved = self.find_direct(profile_id).await
            .expect("Assertion Failed: Profile expected to exist in repository stub");
        check(&saved);
    }

    async fn assert_captured_event_for<F>(&self, profile_id: ProfileId, check: F) 
    where 
        F: FnOnce(&ProfileEvent) + Send 
    {
        let events = self.get_captured_events(profile_id).await;
        assert_eq!(
            events.len(),
            1,
            "Assertion Failed: Expected exactly 1 domain event for profile {:?}", profile_id
        );

        let generic_event = events[0].as_any();
        let profile_event = generic_event
            .downcast_ref::<ProfileEvent>()
            .expect("Assertion Failed: Captured event is not a ProfileEvent");

        check(profile_event);
    }

    async fn assert_no_events_for(&self, profile_id: ProfileId) {
        let events = self.get_captured_events(profile_id).await;
        assert!(
            events.is_empty(),
            "Assertion Failed: Expected 0 events, but captured: {:?}", events
        );
    }
}