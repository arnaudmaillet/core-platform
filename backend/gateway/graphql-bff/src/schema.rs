use crate::domains::profile::{ProfileMutation, ProfileQuery};
use async_graphql::{EmptySubscription, MergedObject, Schema, SchemaBuilder};

#[derive(MergedObject, Default)]
pub struct RootQuery(ProfileQuery);

#[derive(MergedObject, Default)]
pub struct RootMutation(ProfileMutation);

pub async fn build_schema() -> SchemaBuilder<RootQuery, RootMutation, EmptySubscription> {
    Schema::build(
        RootQuery::default(),
        RootMutation::default(),
        EmptySubscription,
    )
}
