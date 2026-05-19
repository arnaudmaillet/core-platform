// common/rust/shared-proto/src/lib.rs

pub mod account {
    pub mod v1 {
        tonic::include_proto!("account.v1");
    }
}

pub mod profile {
    pub mod v1 {
        tonic::include_proto!("profile.v1");
    }
}

pub mod social {
    pub mod v1 {
        tonic::include_proto!("social.v1");
    }
}

pub const SERVICE_DESCRIPTOR_SET: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/service_descriptor.bin"));
