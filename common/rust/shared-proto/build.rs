// common/rust/shared-proto/build.rs

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Définition de la racine (chemin relatif par rapport à build.rs)
    let proto_root = PathBuf::from("../../../proto");

    // 2. Construction des chemins complets vers les fichiers de services
    let services_proto = proto_root.join("account/v1/services.proto");
    let admin_proto = proto_root.join("account/v1/admin.proto");

    // 3. Configuration de Tonic
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(
            PathBuf::from(std::env::var("OUT_DIR")?).join("service_descriptor.bin"),
        )
        .compile_protos(&[services_proto, admin_proto], &[proto_root])?;

    // Indiquer à Cargo de recompiler si les fichiers proto changent
    println!("cargo:rerun-if-changed=../../../proto");

    Ok(())
}
