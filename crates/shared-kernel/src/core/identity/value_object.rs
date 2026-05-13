use crate::core::Result;

pub trait ValueObject: PartialEq + Clone {
    fn validate(&self) -> Result<()>;
}
