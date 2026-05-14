// crates/account/src/lib.rs

mod application;
mod bootstrap;
mod domain;
mod infrastructure;
mod presentation;

pub use bootstrap::AccountServiceBuilder;

pub use domain::{entities, events, repositories, types};

pub use application::commands;
pub use application::context;

pub use infrastructure::{postgres::repositories as db, utils};

pub use presentation::services;

#[cfg(feature = "test-utils")]
pub mod test_utils;
