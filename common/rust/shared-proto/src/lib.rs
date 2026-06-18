// common/rust/shared-proto/src/lib.rs

#[cfg(feature = "account")]
pub mod account {
    pub mod v1 {
        tonic::include_proto!("account.v1");
        include!(concat!(env!("OUT_DIR"), "/account.v1.serde.rs"));
    }
}

#[cfg(feature = "profile")]
pub mod profile {
    pub mod v1 {
        tonic::include_proto!("profile.v1");
        // 👑 Jointure de pbjson : apporte le support Serde natif au DTO
        include!(concat!(env!("OUT_DIR"), "/profile.v1.serde.rs"));
    }
}

#[cfg(feature = "post")]
pub mod post {
    pub mod v1 {
        tonic::include_proto!("post.v1");
        include!(concat!(env!("OUT_DIR"), "/post.v1.serde.rs"));
    }
}

#[cfg(feature = "social")]
pub mod social {
    pub mod v1 {
        tonic::include_proto!("social.v1");
        include!(concat!(env!("OUT_DIR"), "/social.v1.serde.rs"));
    }
}

#[cfg(feature = "geo-discovery")]
pub mod geo_discovery {
    pub mod v1 {
        tonic::include_proto!("geo_discovery.v1");
        include!(concat!(env!("OUT_DIR"), "/geo_discovery.v1.serde.rs"));
    }
}

#[cfg(feature = "comment")]
pub mod comment {
    pub mod v1 {
        tonic::include_proto!("comment.v1");
        include!(concat!(env!("OUT_DIR"), "/comment.v1.serde.rs"));
    }
}

pub const SERVICE_DESCRIPTOR_SET: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/service_descriptor.bin"));
