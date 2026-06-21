fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(
            &[
                "proto/account/v1/enums.proto",
                "proto/account/v1/messages.proto",
                "proto/account/v1/service.proto",
            ],
            &["proto/"],
        )?;
    Ok(())
}
