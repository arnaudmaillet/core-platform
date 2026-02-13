// crates/profile/src/infrastructure/api/grpc/mod.rs
pub mod handlers;
pub mod mappers;

// --- MONDE CARGO / IDE ---
#[cfg(not(bazel))]
pub mod profile_v1_raw_proto {
    pub mod location {
        pub mod v1 {
            include!("proto/location.v1.rs");
        }
    }
    pub mod profile {
        pub mod v1 {
            include!("proto/profile.v1.rs");
            pub const FILE_DESCRIPTOR_SET: &[u8] = include_bytes!("proto/profile_descriptor.bin");
        }
    }
}

// --- MONDE BAZEL ---
#[cfg(bazel)]
// On ré-exporte la crate injectée par Bazel sous le même nom local
pub use profile_v1_raw_proto;

// --- FACADE ---
// Pour simplifier les imports dans tes handlers
pub use profile_v1_raw_proto::location::v1 as location_v1;
pub use profile_v1_raw_proto::profile::v1 as profile_v1;
pub const SERVICE_DESCRIPTOR_SET: &[u8] = profile_v1_raw_proto::profile::v1::FILE_DESCRIPTOR_SET;
