use crate::repositories::CounterRepositoryStub;
use async_trait::async_trait;
use shared_kernel::types::ProfileId;
use social::repositories::CounterRepository;

#[async_trait]
pub trait CounterRepositoryAsserts {
    async fn assert_counters_values(
        &self,
        profile_id: ProfileId,
        expected_followers: u64,
        expected_following: u64,
    );
    fn assert_profile_is_dirty(&self, profile_id: &ProfileId);
}

#[async_trait]
impl CounterRepositoryAsserts for CounterRepositoryStub {
    async fn assert_counters_values(
        &self,
        profile_id: ProfileId,
        expected_followers: u64,
        expected_following: u64,
    ) {
        let res = self.get_counters(profile_id).await.unwrap();

        assert_eq!(
            res.followers_count().value(),
            expected_followers,
            "Assertion Failed: Followers count mismatch pour le profil {}",
            profile_id
        );
        assert_eq!(
            res.following_count().value(),
            expected_following,
            "Assertion Failed: Following count mismatch pour le profil {}",
            profile_id
        );
    }

    fn assert_profile_is_dirty(&self, profile_id: &ProfileId) {
        assert!(
            self.is_profile_dirty(profile_id),
            "Assertion Failed: Le profil {} aurait dû être marqué comme DIRTY (Cache miss sync requis)",
            profile_id
        );
    }
}
