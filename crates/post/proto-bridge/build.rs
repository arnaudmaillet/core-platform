use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")?;

    let proto_root = PathBuf::from(&manifest_dir)
        .join("../proto")
        .canonicalize()?;
    let proto_root_str = proto_root
        .to_str()
        .expect("Chemin des protos non valide UTF-8");

    let base_v1 = proto_root.join("post/v1");
    let protos = &[
        base_v1.join("models.proto"),
        base_v1.join("command.proto"),
        base_v1.join("query.proto"),
    ];

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_with_config(
            prost_build::Config::new(),
            protos,
            &[proto_root.clone(), base_v1.clone()],
        )?;

    println!("cargo:rerun-if-changed={}", proto_root_str);

    Ok(())
}
