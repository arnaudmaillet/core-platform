//! Pure value objects for the realtime delivery model. No I/O, no clock reads
//! (time is injected as `DateTime<Utc>` parameters), no transport or store
//! awareness, no dependency on the generated `realtime-api` types (the proto
//! mapping lives in the infrastructure tier).

pub mod channel;
pub mod identity;
pub mod presence;
pub mod sequence;

pub use channel::{ChannelClass, ChannelKey, ChannelRef, DeliveryGuarantee};
pub use identity::{ConnectionId, DeviceId, NodeId, UserId};
pub use presence::PresenceState;
pub use sequence::{SequenceState, StreamSeq};
