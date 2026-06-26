//! The finalize-time content probe — decode the uploaded bytes for the verified
//! facts (real format, dimensions, size, SHA-256). Never trusts the client's
//! declaration.

pub mod image_media_probe;

pub use image_media_probe::ImageMediaProbe;
