#[cfg(feature = "concurrency")]
mod singleflight;

#[cfg(feature = "concurrency")]
pub use singleflight::Singleflight;
