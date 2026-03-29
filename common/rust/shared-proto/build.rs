// common/rust/shared-proto/build.rs

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = "../../../proto";

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(
            std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("service_descriptor.bin"),
        )
        .compile_protos(
            &[format!("{}/account/v1/account.proto", proto_root)],
            &[proto_root.to_string()],
        )?;

    println!("cargo:rerun-if-changed={}", proto_root);
    Ok(())
}
