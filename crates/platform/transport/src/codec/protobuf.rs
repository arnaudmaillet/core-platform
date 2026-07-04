use bytes::{Bytes, BytesMut};
use prost::Message;

use crate::error::CodecError;

pub fn encode<M: Message>(msg: &M) -> Result<Bytes, CodecError> {
    let mut buf = BytesMut::with_capacity(msg.encoded_len());
    msg.encode(&mut buf).map_err(CodecError::ProtobufEncode)?;
    Ok(buf.freeze())
}

pub fn decode<M: Message + Default>(bytes: &[u8]) -> Result<M, CodecError> {
    M::decode(bytes).map_err(CodecError::ProtobufDecode)
}
