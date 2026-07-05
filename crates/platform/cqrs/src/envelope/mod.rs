#[allow(clippy::module_inception)] // file-per-concept layout: dir + core file share the name
pub(crate) mod envelope;

pub use envelope::*;
