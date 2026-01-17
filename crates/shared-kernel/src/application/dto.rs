// crates/shared-kernel/src/application/dto.rs
use crate::errors::Result;

/// Permet de convertir un DTO (Data Transfer Object) en objet de Domaine
/// tout en gérant les erreurs de validation proprement.
pub trait FromDto<D> {
    fn from_dto(dto: D) -> Result<Self> where Self: Sized;
}

pub trait ToDto<D> {
    fn to_dto(&self) -> D;
}