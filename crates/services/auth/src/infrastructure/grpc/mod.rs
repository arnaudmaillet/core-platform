//! gRPC inbound adapter: maps the `auth.v1` contract onto the application
//! handlers. The generated server trait is implemented in [`server`]; the
//! per-RPC translation lives in [`handler`].

pub mod handler;
pub mod server;
