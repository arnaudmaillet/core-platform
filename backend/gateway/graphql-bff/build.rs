// backend/gateway/graphql-bff/build.rs

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = "../../../proto";
    let out_dir = "src/infrastructure/api/grpc/proto";

    std::fs::create_dir_all(out_dir)?;

    tonic_prost_build::configure()
        .out_dir(out_dir)
        .compile_protos(
            &[
                format!("{}/profile/v1/profile.proto", proto_root),
                format!("{}/profile/v1/profile_query.proto", proto_root),
                format!("{}/profile/v1/user_location.proto", proto_root),
            ],
            &[proto_root.to_string()],
        )?;

    println!("cargo:rerun-if-changed={}", proto_root);

    Ok(())
}
