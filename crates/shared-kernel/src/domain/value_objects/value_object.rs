use crate::errors::Result;

pub trait ValueObject: PartialEq + Clone {
    fn validate(&self) -> Result<()>;
}