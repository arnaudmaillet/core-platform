// crates/profile/build.rs

use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = "src/infrastructure/api/grpc/proto";
    let proto_root = "../../proto";

    std::fs::create_dir_all(out_dir)?;

    tonic_prost_build::configure()
        .out_dir(out_dir)
        .compile_protos(
            &[
                format!("{}/profile/v1/types.proto", proto_root),
                format!("{}/profile/v1/profile.proto", proto_root),
                format!("{}/profile/v1/profile_query.proto", proto_root),
                format!("{}/profile/v1/user_location.proto", proto_root),
            ],
            &[proto_root.to_string()], // Correction du type ici
        )?;

    println!("cargo:rerun-if-changed={}", proto_root);
    Ok(())
}