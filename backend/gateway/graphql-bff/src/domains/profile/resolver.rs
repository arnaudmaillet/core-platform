// backend/gateway/graphql-bff/src/domains/profile/resolver.rs

use crate::context::ApiContext;
use crate::domains::profile::model::Profile;
use async_graphql::{Context, Object, Result};
use crate::clients::profile::UpdateHandleRequest;

#[derive(Default)]
pub struct ProfileQuery;

#[Object]
impl ProfileQuery {
    // Exemple : Récupérer un profil par son username (via le QueryService)
    async fn get_profile(&self, ctx: &Context<'_>, username: String) -> Result<Profile> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let mut client = api_ctx.profile_query.clone();

        // On imagine une méthode gRPC SearchProfiles ou GetByUsername
        // Pour l'exemple, on simule l'appel
        let request = tonic::Request::new(crate::clients::profile::SearchProfilesRequest {
            query: username,
            page_size: 1,
            ..Default::default()
        });

        let response = client.search_profiles(request).await?.into_inner();

        if let Some(first) = response.results.first() {
            // Ici tu appellerais normalement un GetProfile complet si le summary ne suffit pas
            // Pour l'instant, on mappe ce qu'on a
            // ...
        }

        Err("Profile not found".into())
    }
}

#[derive(Default)]
pub struct ProfileMutation;

#[Object]
impl ProfileMutation {
    async fn update_handle(
        &self,
        ctx: &Context<'_>,
        profile_id: String,
        new_handle: String,
    ) -> Result<Profile> {
        let api_ctx = ctx.data::<ApiContext>()?;
        let mut client = api_ctx.profile_identity.clone();

        let request = tonic::Request::new(UpdateHandleRequest {
            profile_id,
            new_handle,
        });

        let response = client.update_handle(request).await?.into_inner();
        Ok(Profile::from(response))
    }
}
