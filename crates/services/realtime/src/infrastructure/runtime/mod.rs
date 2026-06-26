//! The serving runtime: the node-local connection table, the gateway WS edge, and
//! the dispatcher fan-out consumer. Composed into the two `service_runtime::Service`
//! impls in [`crate::service`].

pub mod connection_table;
pub mod dispatcher;
pub mod gateway;

pub use connection_table::{ConnHandle, ConnectionTable};
pub use dispatcher::run_fanout_consumer;
pub use gateway::{GatewayState, serve_ws, spawn_node_subscriber};
