fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(
            &[
                "proto/profile/v1/enums.proto",
                "proto/profile/v1/messages.proto",
                "proto/profile/v1/service.proto",
            ],
            &["proto/"],
        )?;
    Ok(())
}
