//! Account-directory adapter — the small synchronous slice moderation needs from
//! `account` (confirming an actor exists before an actor-level enforcement).

pub mod grpc_account_directory;

pub use grpc_account_directory::GrpcAccountDirectory;
