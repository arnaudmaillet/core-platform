// crates/profile/src/context/command_context.rs

use crate::{
    context::ProfileAppContext, entities::Profile, repositories::ProfileRoutingRepository,
    types::Handle,
};
use shared_kernel::{
    command::CommandTarget,
    core::{Error, Result, Versioned},
    types::{ProfileId, Region},
};
use std::sync::Arc;

#[derive(Clone)]
pub struct ProfileCommandContext {
    app: ProfileAppContext,
    profile_id: Option<ProfileId>,
}

impl ProfileCommandContext {
    pub(crate) fn new(app: ProfileAppContext, profile_id: Option<ProfileId>) -> Self {
        Self { app, profile_id }
    }

    pub fn routing_repo(&self) -> Arc<dyn ProfileRoutingRepository> {
        self.app.routing_repo()
    }

    pub fn local_region(&self) -> Region {
        self.app.local_region()
    }

    pub async fn exists_by_handle(&self, handle: &Handle) -> Result<bool> {
        let slug_hash = handle.to_sha256_hash();
        let res = self.routing_repo().resolve_slug(&slug_hash).await?;
        Ok(res.is_some())
    }

    pub async fn fetch_verified(&self, target: &CommandTarget<ProfileId>) -> Result<Profile> {
        if Some(&target.id) != self.profile_id.as_ref() {
            return Err(Error::validation("target", "Context/Target mismatch"));
        }

        let profile = self
            .app
            .profile_repo()
            .find_by_id(target.id)
            .await?
            .ok_or_else(|| Error::not_found("Profile", target.id.to_string()))?;

        let expected_version = target
            .expected_version
            .ok_or_else(|| Error::validation("expected_version", "Expected version is missing"))?;

        if profile.version() != expected_version {
            return Err(Error::concurrency_conflict(format!(
                "OCC Mismatch: DB v{}, Expected v{}",
                profile.version(),
                expected_version
            )));
        }

        Ok(profile)
    }

    pub async fn save(&self, profile: &mut Profile) -> Result<()> {
        if let Some(expected_id) = self.profile_id {
            if profile.profile_id() != expected_id {
                return Err(Error::validation(
                    "profile_id",
                    "Identity mismatch violation",
                ));
            }
        }

        self.app.profile_repo().save(profile).await?;

        Ok(())
    }
}
