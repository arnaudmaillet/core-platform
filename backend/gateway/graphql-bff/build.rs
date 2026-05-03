// backend/gateway/graphql-bff/build.rs

use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let proto_root = manifest_dir.join("../../../proto").canonicalize()?;
    let out_dir = manifest_dir.join("src/infrastructure/api/grpc/proto");

    std::fs::create_dir_all(&out_dir)?;

    tonic_prost_build::configure()
        .out_dir(&out_dir)
        .compile_protos(
            &[
                "profile/v1/profile.proto",
                "profile/v1/profile_query.proto",
                "profile/v1/user_location.proto",
            ],
            &[proto_root.to_str().expect("Invalid UTF-8 in path")],
        )?;

    println!("cargo:rerun-if-changed={}", proto_root.display());
    Ok(())
}
