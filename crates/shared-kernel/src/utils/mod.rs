// crates/shared-kernel/src/utils/mod.rs

#[cfg(feature = "test-utils")]
pub mod cache_repository_stub;

#[cfg(feature = "test-utils")]
pub use cache_repository_stub::CacheRepositoryStub;
