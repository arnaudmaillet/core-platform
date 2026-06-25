//! Compiles the moderation.v1 contract into server + client stubs plus a
//! reflection descriptor set, from the shared contracts/proto root (the single
//! IDL source).
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let descriptor_path =
        std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("moderation_descriptor.bin");

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(&descriptor_path)
        .compile_protos(
            &[
                "../proto/moderation/v1/enums.proto",
                "../proto/moderation/v1/messages.proto",
                "../proto/moderation/v1/service.proto",
            ],
            &["../proto/"],
        )?;
    Ok(())
}
