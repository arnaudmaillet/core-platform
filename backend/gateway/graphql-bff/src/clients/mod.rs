// backend/gateway/graphql-bff/src/clients/mod.rs

pub mod profile {
    tonic::include_proto!("profile.v1");
}

pub mod location {
    tonic::include_proto!("location.v1");
}