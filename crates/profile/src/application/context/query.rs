// crates/profile/src/context/query_context.rs

use crate::{context::ProfileKernelCtx, entities::Profile, types::Handle};
use shared_kernel::{
    core::{Error, Result},
    types::{ProfileId, Region},
};

pub struct ProfileQueryCtx {
    kernel: ProfileKernelCtx,
}

impl Clone for ProfileQueryCtx {
    fn clone(&self) -> Self {
        Self {
            kernel: self.kernel.clone(),
        }
    }
}

impl ProfileQueryCtx {
    pub fn new(kernel: ProfileKernelCtx) -> Self {
        Self { kernel }
    }

    pub async fn find_by_id(&self, profile_id: ProfileId) -> Result<Option<Profile>> {
        self.kernel.profile_repo().find_by_id(profile_id).await
    }

    pub async fn find_by_handle(&self, handle: &Handle) -> Result<Option<Profile>> {
        let slug_hash = handle.to_sha256_hash();

        if let Some((profile_id, target_region)) =
            self.kernel.routing_repo().resolve_slug(&slug_hash).await?
        {
            if target_region == self.kernel.server_region() {
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
