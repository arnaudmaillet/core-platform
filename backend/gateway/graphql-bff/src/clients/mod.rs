// backend/gateway/graphql-bff/src/clients/mod.rs

pub mod profile {
    include!("../infrastructure/api/grpc/proto/profile.v1.rs");
}

pub mod location {
    include!("../infrastructure/api/grpc/proto/location.v1.rs");
}