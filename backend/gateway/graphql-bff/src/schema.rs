use async_graphql::{MergedObject, Schema, EmptySubscription, SchemaBuilder};
use crate::domains::profile::{ProfileQuery, ProfileMutation};

#[derive(MergedObject, Default)]
pub struct RootQuery(ProfileQuery);

#[derive(MergedObject, Default)]
pub struct RootMutation(ProfileMutation);

pub type AppSchema = Schema<RootQuery, RootMutation, EmptySubscription>;

pub async fn build_schema() -> SchemaBuilder<RootQuery, RootMutation, EmptySubscription> {
    Schema::build(RootQuery::default(), RootMutation::default(), EmptySubscription)
}