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

    if env::var("CARGO_FEATURE_ACCOUNT").is_ok() {
        protos.push("account/v1/service.proto");
        protos.push("account/v1/models.proto");
    }
    if env::var("CARGO_FEATURE_PROFILE").is_ok() {
        protos.push("profile/v1/service.proto");
        protos.push("profile/v1/models.proto");
    }
    if env::var("CARGO_FEATURE_SOCIAL").is_ok() {
        protos.push("social/v1/service.proto");
    }
    if env::var("CARGO_FEATURE_POST").is_ok() {
        protos.push("post/v1/models.proto");
        protos.push("post/v1/service.proto");
    }
    if protos.is_empty() {
        return Ok(());
    }

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(
            PathBuf::from(env::var("OUT_DIR")?).join("service_descriptor.bin"),
        )
        .compile_protos(&protos, &[proto_root_str])?;

    println!("cargo:rerun-if-changed={}", proto_root_str);

    Ok(())
}
