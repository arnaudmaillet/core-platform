// crates/profile/src/application/context_builder.rs

use crate::application::context::{ProfileAppContext, ProfileContext};
use crate::{repositories::ProfileRepository, value_objects::ProfileId};
use shared_kernel::types::RegionCode;
use std::sync::Arc;

pub struct ProfileContextBuilder {
    app: Option<ProfileAppContext>,
    profile_id: Option<ProfileId>,
    region: Option<RegionCode>,
}

impl ProfileContextBuilder {
    pub fn new() -> Self {
        Self {
            app: None,
            profile_id: None,
            region: None,
        }
    }

    pub fn with_app(mut self, app: ProfileAppContext) -> Self {
        self.app = Some(app);
        self
    }

    pub fn with_profile_id(mut self, id: ProfileId) -> Self {
        self.profile_id = Some(id);
        self
    }

    pub fn with_region(mut self, region: RegionCode) -> Self {
        self.region = Some(region);
        self
    }

    // Getters utiles pour les Middlewares ou Tests
    pub fn profile_id(&self) -> Option<&ProfileId> {
        self.profile_id.as_ref()
    }

    pub fn region(&self) -> Option<&RegionCode> {
        self.region.as_ref()
    }

    pub fn profile_repo(&self) -> Option<Arc<dyn ProfileRepository>> {
        self.app.as_ref().map(|a| a.profile_repo())
    }

    pub fn build(self) -> ProfileContext {
        let app = self
            .app
            .expect("ProfileAppContext is required. Use .with_app()");
        let profile_id = self
            .profile_id
            .expect("profile_id is required for ProfileContext");
        let region = self.region.expect("region is required for ProfileContext");

        ProfileContext::new(app, profile_id, region)
    }
}

impl Default for ProfileContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}
