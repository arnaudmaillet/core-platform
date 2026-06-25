//! Classifier-gateway adapters. The real classifier services are a future,
//! internal dependency; until they exist this is a log stub, and the graduated
//! engine runs on deterministic rules alone.

pub mod log_classifier_gateway;

pub use log_classifier_gateway::LogClassifierGateway;
