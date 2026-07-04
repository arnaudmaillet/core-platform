//! The transformation engine (Plane B) for images: derive the resize ladder + a
//! BlurHash from the validated master, writing each content-addressed derivative
//! back to the object store.

pub mod image_rendition_processor;

pub use image_rendition_processor::ImageRenditionProcessor;
