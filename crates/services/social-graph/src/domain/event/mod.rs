pub mod profile_blocked;
pub mod profile_followed;
pub mod profile_unblocked;
pub mod profile_unfollowed;

pub use profile_blocked::ProfileBlocked;
pub use profile_followed::ProfileFollowed;
pub use profile_unblocked::ProfileUnblocked;
pub use profile_unfollowed::ProfileUnfollowed;

#[derive(Debug, Clone)]
pub enum DomainEvent {
    ProfileFollowed(ProfileFollowed),
    ProfileUnfollowed(ProfileUnfollowed),
    ProfileBlocked(ProfileBlocked),
    ProfileUnblocked(ProfileUnblocked),
}
