// proto/identity/v1/build.rs

fn main() {
    tonic_prost_build::configure()
        .build_server(true)  // important pour générer le server code
        .build_client(true)  // si tu veux aussi le client
        .compile_protos(&["user.proto"], &["."])
        .unwrap_or_else(|e| panic!("Failed to compile protos: {}", e));
}