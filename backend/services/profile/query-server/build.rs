// backend/services/profile/query-server/build.rs

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .out_dir("src/")
        .compile(
            &[
                "../../../../proto/profile/v1/profile.proto",
                "../../../../proto/profile/v1/profile_query.proto",
                "../../../../proto/profile/v1/user_location.proto",
            ],
            &["../../../../proto/"]
        )?;
    Ok(())
}