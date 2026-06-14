// crates/social-test-utils/src/assertions/relation_assert.rs

use crate::repositories::RelationRepositoryStub;
use async_trait::async_trait;
use shared_kernel::types::ProfileId;
use social::events::SocialEvent;
use social::repositories::FollowRelationRepository;

#[async_trait]
pub trait RelationRepositoryAsserts {
    async fn assert_relation_exists(&self, follower_id: ProfileId, following_id: ProfileId);
    async fn assert_relation_does_not_exist(&self, follower_id: ProfileId, following_id: ProfileId);

    // 💡 Signatures calquées sur Profile
    async fn assert_captured_event_for<F>(&self, follower_id: ProfileId, check: F)
    where
        F: FnOnce(&SocialEvent) + Send;
    async fn assert_no_events_for(&self, follower_id: ProfileId);
}

#[async_trait]
impl RelationRepositoryAsserts for RelationRepositoryStub {
    async fn assert_relation_exists(&self, follower_id: ProfileId, following_id: ProfileId) {
        let exists = self.is_following(follower_id, following_id).await.unwrap();
        assert!(
            exists,
            "Assertion Failed: La relation [{} -> {}] aurait dû exister",
            follower_id, following_id
        );
    }

    async fn assert_relation_does_not_exist(
        &self,
        follower_id: ProfileId,
        following_id: ProfileId,
    ) {
        let exists = self.is_following(follower_id, following_id).await.unwrap();
        assert!(
            !exists,
            "Assertion Failed: La relation [{} -> {}] n'aurait PAS dû exister",
            follower_id, following_id
        );
    }

    // 💡 Va chercher l'événement lié spécifiquement à ce follower_id
    async fn assert_captured_event_for<F>(&self, follower_id: ProfileId, check: F)
    where
        F: FnOnce(&SocialEvent) + Send,
    {
        let events = self.get_captured_events_for(follower_id);

        assert!(
            !events.is_empty(),
            "Assertion Failed: Aucun événement de domaine capturé pour le follower {:?}",
            follower_id
        );

        let generic_event = events[0].as_any();
        let social_event = generic_event
            .downcast_ref::<SocialEvent>()
            .expect("Assertion Failed: L'événement n'est pas un SocialEvent");

        check(social_event);
    }

    async fn assert_no_events_for(&self, follower_id: ProfileId) {
        let events = self.get_captured_events_for(follower_id);
        assert!(
            events.is_empty(),
            "Assertion Failed: Attendu 0 événements pour le follower {:?}, reçu: {}",
            follower_id,
            events.len()
        );
    }
}
