#[cfg(test)]
pub mod profile_repository_stub;
pub mod profile_stats_repository_stub;
pub mod user_location_repository_stub;

#[cfg(test)]
pub use profile_repository_stub::{FakeTransaction, OutboxRepoStub, ProfileRepositoryStub};
pub use profile_stats_repository_stub::ProfileStatsRepositoryStub;
pub use user_location_repository_stub::LocationRepositoryStub;
