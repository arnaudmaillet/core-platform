// common/rust/shared-proto/src/lib.rs

#[cfg(feature = "account")]
pub mod account {
    pub mod v1 {
        tonic::include_proto!("account.v1");
    }
}

#[cfg(feature = "profile")]
pub mod profile {
    pub mod v1 {
        tonic::include_proto!("profile.v1");
    }
}

#[cfg(feature = "post")]
pub mod post {
    pub mod v1 {
        tonic::include_proto!("post.v1");
    }
}

#[cfg(feature = "social")]
pub mod social {
    pub mod v1 {
        tonic::include_proto!("social.v1");
    }
}

#[cfg(feature = "geo-discovery")]
pub mod geo_discovery {
    pub mod v1 {
        tonic::include_proto!("geo_discovery.v1");
    }
}

pub const SERVICE_DESCRIPTOR_SET: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/service_descriptor.bin"));
