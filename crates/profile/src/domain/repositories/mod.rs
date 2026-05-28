mod repository;
pub use repository::ProfileRepository;

#[cfg(test)]
mod stub;
#[cfg(test)]
pub use stub::ProfileRepositoryStub;
