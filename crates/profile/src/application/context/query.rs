// crates/profile/src/context/query_context.rs

use crate::{context::ProfileAppContext, entities::Profile, types::Handle};
use shared_kernel::{
    core::Result,
    types::{ProfileId, Region},
};

pub struct ProfileQueryContext<TM> {
    app_ctx: ProfileAppContext<TM>,
    region: Region,
}

impl<TM> Clone for ProfileQueryContext<TM> {
    fn clone(&self) -> Self {
        Self {
            app_ctx: self.app_ctx.clone(),
            region: self.region,
        }
    }
}

impl<TM> ProfileQueryContext<TM> {
    pub(crate) fn new(app_ctx: ProfileAppContext<TM>, region: Region) -> Self {
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
