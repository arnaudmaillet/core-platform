fn main() -> Result<(), Box<dyn std::error::Error>> {
    let descriptor_path =
        std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("social_graph_descriptor.bin");

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(false)
        .file_descriptor_set_path(&descriptor_path)
        .compile_protos(
            &[
                "proto/social_graph/v1/enums.proto",
                "proto/social_graph/v1/messages.proto",
                "proto/social_graph/v1/service.proto",
            ],
            &["proto/"],
        )?;
    Ok(())
}
