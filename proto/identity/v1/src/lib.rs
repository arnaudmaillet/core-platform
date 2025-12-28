// proto/identity/v1/src/lib.rs

pub mod identity {
    tonic::include_proto!("identity.v1"); 
}

// Ré-export pour faciliter l'usage
pub use identity::*;