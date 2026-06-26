//! The byte-plane adapter: an S3-compatible object store (S3 / MinIO).
//!
//! [`S3Client`] mints pre-signed URLs (the client PUTs bytes straight to the
//! store, off the mesh) and also performs the server-side GET/PUT the worker needs
//! to probe and derive renditions. [`S3ObjectStore`] adapts it to the
//! [`ObjectStore`](crate::application::port::ObjectStore) port. rusty-s3 builds the
//! signed URLs; reqwest executes the HTTP.

pub mod config;
pub mod s3_client;
pub mod s3_object_store;

pub use config::S3Config;
pub use s3_client::S3Client;
pub use s3_object_store::S3ObjectStore;
