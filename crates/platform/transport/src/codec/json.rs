use serde::{de::DeserializeOwned, Serialize};

use crate::error::CodecError;

pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, CodecError> {
    serde_json::to_vec(value).map_err(CodecError::Json)
}

pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, CodecError> {
    serde_json::from_slice(bytes).map_err(CodecError::Json)
}
