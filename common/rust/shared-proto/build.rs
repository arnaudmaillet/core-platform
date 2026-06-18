// common/rust/shared-proto/build.rs

use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")?;
    let proto_root = PathBuf::from(manifest_dir)
        .join("../../../proto")
        .canonicalize()?;

    let proto_root_str = proto_root.to_str().expect("Chemin non valide UTF-8");
    let mut protos = Vec::new();
    let mut serde_packages = Vec::new();

    if env::var("CARGO_FEATURE_ACCOUNT").is_ok() {
        protos.push("account/v1/service.proto");
        protos.push("account/v1/models.proto");
        serde_packages.push(".account.v1");
    }
    if env::var("CARGO_FEATURE_PROFILE").is_ok() {
        protos.push("profile/v1/service.proto");
        protos.push("profile/v1/models.proto");
        serde_packages.push(".profile.v1");
    }
    if env::var("CARGO_FEATURE_SOCIAL").is_ok() {
        protos.push("social/v1/service.proto");
        serde_packages.push(".social.v1");
    }
    if env::var("CARGO_FEATURE_POST").is_ok() {
        protos.push("post/v1/models.proto");
        protos.push("post/v1/service.proto");
        serde_packages.push(".post.v1");
    }
    if env::var("CARGO_FEATURE_POST").is_ok() {
        protos.push("geo_discovery/v1/models.proto");
        protos.push("geo_discovery/v1/service.proto");
        serde_packages.push(".geo_discovery.v1");
    }
    if env::var("CARGO_FEATURE_POST").is_ok() {
        protos.push("comment/v1/models.proto");
        protos.push("comment/v1/service.proto");
        serde_packages.push(".comment.v1");
    }

    if protos.is_empty() {
        return Ok(());
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let descriptor_path = out_dir.join("service_descriptor.bin");

    // 1. Configuration et compilation gRPC via ton wrapper
    let mut prost_config = prost_build::Config::new();
    prost_config.extern_path(".google.protobuf.Timestamp", "::pbjson_types::Timestamp");

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(&descriptor_path)
        .compile_with_config(prost_config, &protos, &[proto_root_str])?;

    // 2. Lecture du descripteur binaire généré
    let descriptor_set = std::fs::read(&descriptor_path)?;

    // 3. Configuration de pbjson
    let mut pbjson_builder = pbjson_build::Builder::new();
    pbjson_builder.register_descriptors(&descriptor_set)?;

    // Aligner également pbjson sur pbjson_types
    pbjson_builder.extern_path(".google.protobuf.Timestamp", "::pbjson_types::Timestamp");

    // Génération pour tes packages métiers actifs
    for package in &serde_packages {
        pbjson_builder.build(&[package])?;
    }

    println!("cargo:rerun-if-changed={}", proto_root_str);

    Ok(())
}
