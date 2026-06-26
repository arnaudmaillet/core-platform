//! Compiles the realtime.v1 contract into server + client stubs plus a reflection
//! descriptor set, from the shared contracts/proto root (the single IDL source).
//!
//! Note: this contract has two audiences. The `ClientFrame` / `ServerFrame`
//! messages are the client-facing WSS transport envelope (encoded with prost,
//! framed over WebSocket — no gRPC to the client); the `RealtimeDispatchService`
//! is the internal node-hop surface. Both are generated from the same module.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let descriptor_path =
        std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("realtime_descriptor.bin");

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(&descriptor_path)
        .compile_protos(
            &[
                "../proto/realtime/v1/enums.proto",
                "../proto/realtime/v1/messages.proto",
                "../proto/realtime/v1/service.proto",
            ],
            &["../proto/"],
        )?;
    Ok(())
}
