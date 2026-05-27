use crate::{context::ProfileAppContext, entities::Profile, types::Handle};
use shared_kernel::{
    core::Result,
    types::{ProfileId, Region},
};

#[derive(Clone)]
pub struct ProfileQueryContext {
    app_ctx: ProfileAppContext,
    region: Region,
}

impl ProfileQueryContext {
    pub(crate) fn new(app_ctx: ProfileAppContext, region: Region) -> Self {
        Self { app_ctx, region }
    }

    pub fn region(&self) -> Region {
        self.region
    }

    pub async fn find_by_id(&self, profile_id: ProfileId) -> Result<Option<Profile>> {
        self.app_ctx
            .profile_repo()
            .find_by_id(profile_id, self.region, None)
            .await
    }

    pub async fn find_by_handle(&self, handle: &Handle) -> Result<Option<Profile>> {
        self.app_ctx
            .profile_repo()
            .find_by_handle(handle, self.region, None)
            .await
    }
}
