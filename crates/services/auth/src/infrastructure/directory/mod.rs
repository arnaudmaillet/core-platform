//! `account` service adapter for the [`AccountDirectory`](crate::application::port::AccountDirectory)
//! port — a gRPC client over the `account.v1` contract. Auth reads identity here;
//! it never writes it.

pub mod grpc_account_directory;
pub mod grpc_profile_directory;

pub use grpc_account_directory::GrpcAccountDirectory;
pub use grpc_profile_directory::GrpcProfileDirectory;
