fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR")?);

    // ── Timeline service protos (server stubs) ────────────────────────────────
    let timeline_descriptor = out_dir.join("timeline_descriptor.bin");

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(false)
        .file_descriptor_set_path(&timeline_descriptor)
        .compile_protos(
            &[
                "proto/timeline/v1/enums.proto",
                "proto/timeline/v1/messages.proto",
                "proto/timeline/v1/service.proto",
            ],
            &["proto/"],
        )?;

    // Social-graph client stubs now come from the `social-graph-api` crate
    // (contracts tier) — no cross-service proto recompilation here.

    Ok(())
}
