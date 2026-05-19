// common/rust/shared-proto/build.rs

use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")?;
    let proto_root = PathBuf::from(manifest_dir)
        .join("../../../proto")
        .canonicalize()?;

    let protos = [
        "account/v1/access.proto",
        "account/v1/personal.proto",
        "account/v1/settings.proto",
        "account/v1/moderation.proto",
        "account/v1/query.proto",
        "profile/v1/profile.proto",
        "profile/v1/models.proto",
        "social/v1/social.proto",
    ];

    let proto_root_str = proto_root.to_str().expect("Chemin non valide UTF-8");

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
