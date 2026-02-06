mod profile_identity_repository;
mod profile_repository;
mod profile_stats_repository;
mod user_location_repository;

pub use profile_identity_repository::ProfileIdentityRepository;
pub use profile_repository::ProfileRepository;
pub use profile_stats_repository::ProfileStatsRepository;
pub use user_location_repository::LocationRepository;


#[cfg(test)]
mod profile_stats_repository_stub;
#[cfg(test)]
mod user_location_repository_stub;
#[cfg(test)]
mod profile_identity_repository_stub;


#[cfg(test)]
pub use profile_stats_repository_stub::ProfileStatsRepositoryStub;
#[cfg(test)]
pub use user_location_repository_stub::LocationRepositoryStub;
#[cfg(test)]
pub use profile_identity_repository_stub::ProfileRepositoryStub;