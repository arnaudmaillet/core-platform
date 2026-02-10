mod identity_repository;
mod profile_repository;
mod stats_repository;
mod user_location_repository;

pub use identity_repository::ProfileIdentityRepository;
pub use profile_repository::ProfileRepository;
pub use stats_repository::ProfileStatsRepository;
pub use user_location_repository::LocationRepository;


#[cfg(test)]
mod stats_repository_stub;
#[cfg(test)]
mod user_location_repository_stub;
#[cfg(test)]
mod identity_repository_stub;


#[cfg(test)]
pub use stats_repository_stub::ProfileStatsRepositoryStub;
#[cfg(test)]
pub use user_location_repository_stub::LocationRepositoryStub;
#[cfg(test)]
pub use identity_repository_stub::ProfileRepositoryStub;