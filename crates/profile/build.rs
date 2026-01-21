// crates/profile/build.rs

use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = "src/infrastructure/api/grpc/proto";
    let proto_root = "../../proto";

    // On s'assure que le dossier existe pour éviter une erreur de build
    if !Path::new(out_dir).exists() {
        fs::create_dir_all(out_dir)?;
    }

    tonic_build::configure()
        .out_dir(out_dir) // On cible le dossier spécifique
        .compile(
            &[
                format!("{}/profile/v1/types.proto", proto_root),
                format!("{}/profile/v1/profile.proto", proto_root),
                format!("{}/profile/v1/profile_query.proto", proto_root),
                format!("{}/profile/v1/user_location.proto", proto_root),
            ],
            &[proto_root]
        )?;

    // On demande à Cargo de ne pas rebuild inutilement,
    // sauf si un fichier .proto dans ce dossier change
    println!("cargo:rerun-if-changed={}", proto_root);

    Ok(())
}