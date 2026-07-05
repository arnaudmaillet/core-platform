#[allow(clippy::module_inception)] // file-per-concept layout: dir + core file share the name
pub mod principal;

pub use principal::{CurrentPrincipal, Permission, PrincipalId};
