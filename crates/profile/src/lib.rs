mod application;
mod bootstrap;
mod domain;
mod infrastructure;
mod presentation;

pub use application::{commands, context};
pub use bootstrap::ProfileServiceBuilder;
pub use domain::{entities, events, repositories, types};
pub use infrastructure::{kafka, repositories as repositories_impl};
pub use presentation::services;

#[cfg(feature = "test-utils")]
pub mod test_utils;
