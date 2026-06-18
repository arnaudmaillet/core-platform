mod entity;
mod projections;

pub use entity::ProjectedProfile;
pub use projections::{ProfileReadProjection, ProfileWriteProjection};

#[cfg(feature = "test-utils")]
pub use projections::ProfileProjectionStub;
