// common/rust/shared-proto/build.rs

// common/rust/shared-proto/build.rs

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Définition de la racine
    let proto_root = PathBuf::from("../../../proto");

    // 2. Construction des chemins vers les nouveaux fichiers de services
    let protos = [
        proto_root.join("account/v1/access.proto"),
        proto_root.join("account/v1/personal.proto"),
        proto_root.join("account/v1/settings.proto"),
        proto_root.join("account/v1/moderation.proto"),
        proto_root.join("account/v1/query.proto"),
    ];

    // 3. Configuration de Tonic
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(
            PathBuf::from(std::env::var("OUT_DIR")?).join("service_descriptor.bin"),
        )
        // Note : On ne liste pas models.proto ou enums.proto ici car
        // ils sont importés automatiquement par les autres.
        .compile_protos(&protos, &[proto_root])?;

    // Indiquer à Cargo de recompiler si le dossier proto change
    println!("cargo:rerun-if-changed=../../../proto");

    Ok(())
}
