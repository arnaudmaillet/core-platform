// crates/profile/src/context/query_context.rs

use crate::{context::ProfileAppContext, entities::Profile, types::Handle};
use shared_kernel::{
    core::{Error, Result},
    types::{ProfileId, Region},
};

pub struct ProfileQueryContext {
    app_ctx: ProfileAppContext,
}

impl Clone for ProfileQueryContext {
    fn clone(&self) -> Self {
        Self {
            app_ctx: self.app_ctx.clone(),
        }
    }
}

impl ProfileQueryContext {
    pub(crate) fn new(app_ctx: ProfileAppContext) -> Self {
        Self { app_ctx }
    }

    pub fn local_region(&self) -> Region {
        self.app_ctx.local_region()
    }

    pub async fn find_by_id(&self, profile_id: ProfileId) -> Result<Option<Profile>> {
        self.app_ctx.profile_repo().find_by_id(profile_id).await
    }

    pub async fn find_by_handle(&self, handle: &Handle) -> Result<Option<Profile>> {
        let slug_hash = handle.to_sha256_hash();

        if let Some((profile_id, target_region)) =
            self.app_ctx.routing_repo().resolve_slug(&slug_hash).await?
        {
            if target_region == self.local_region() {
                self.find_by_id(profile_id).await
            } else {
                Err(Error::validation(
                    "region",
                    format!(
                        "Profile located in another region ({:?}). Route your gRPC client to the correct endpoint.",
                        target_region
                    ),
                ))
            }
        } else {
            Ok(None)
        }
    }
}
