pub mod handle_changed;
pub mod profile_created;
pub mod profile_deleted;
pub mod profile_hidden;
pub mod profile_restored;
pub mod profile_updated;
pub mod profile_verified;

pub use handle_changed::HandleChanged;
pub use profile_created::ProfileCreated;
pub use profile_deleted::ProfileDeleted;
pub use profile_hidden::ProfileHidden;
pub use profile_restored::ProfileRestored;
pub use profile_updated::ProfileUpdated;
pub use profile_verified::ProfileVerified;

#[derive(Debug, Clone)]
pub enum DomainEvent {
    ProfileCreated(ProfileCreated),
    ProfileUpdated(ProfileUpdated),
    HandleChanged(HandleChanged),
    ProfileHidden(ProfileHidden),
    ProfileRestored(ProfileRestored),
    ProfileVerified(ProfileVerified),
    ProfileDeleted(ProfileDeleted),
}
