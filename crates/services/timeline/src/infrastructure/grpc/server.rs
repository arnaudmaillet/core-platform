//! Server-side proto artifacts for the timeline gRPC surface.
//!
//! The deployable binary boots through the fleet runtime
//! (`service_runtime::serve::<TimelineService>` → [`crate::service::TimelineService`]),
//! which owns config loading, hot-reload, observability, and the layer stack. This
//! module is therefore reduced to the one artifact that path still needs: the embedded
//! file-descriptor set used to register server reflection.

/// Proto file descriptor blob embedded at build time for server reflection.
pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("timeline_descriptor");
