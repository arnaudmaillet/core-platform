//! The delivery-plane adapter: turn a content-addressed key into a CDN URL.
//!
//! Public media gets a stable, immutable URL off the CDN base; private media gets a
//! short-lived signed URL minted via the object store. Edge invalidation is a
//! takedown-only path (a real CloudFront `CreateInvalidation` is a Phase-7/ops
//! follow-up; this logs).

pub mod cloudfront_cdn_gateway;

pub use cloudfront_cdn_gateway::CloudFrontCdnGateway;
