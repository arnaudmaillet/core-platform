mod metadata_repository;
mod identity_repository;
mod settings_repository;

pub use metadata_repository::AccountMetadataRepository;
pub use identity_repository::AccountIdentityRepository;
pub use settings_repository::AccountSettingsRepository;


#[cfg(test)]
mod identity_repository_stub;
#[cfg(test)]
mod metadata_repository_stub;
#[cfg(test)]
mod settings_repository_stub;

#[cfg(test)]
pub use identity_repository_stub::AccountIdentityRepositoryStub;
#[cfg(test)]
pub use metadata_repository_stub::AccountMetadataRepositoryStub;
#[cfg(test)]
pub use settings_repository_stub::AccountSettingsRepositoryStub;
