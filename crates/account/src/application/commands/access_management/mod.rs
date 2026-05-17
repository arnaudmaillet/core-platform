pub mod link_sub_identity;
pub mod register;
pub mod resolve_identity;

pub use link_sub_identity::link_sub_identity_command::LinkSubIdentityCommand;
pub use link_sub_identity::link_sub_identity_handle::LinkSubIdentityHandler;
pub use register::register_command::RegisterCommand;
pub use register::register_handler::RegisterHandler;
