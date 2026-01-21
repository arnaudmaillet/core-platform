// backend/gateway/graphql-bff/build.rs

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. On pointe vers la racine du dossier proto (celle qui contient les dossiers 'profile' et 'location')
    let proto_root = "../../../proto";

    tonic_build::configure()
        .compile(
            &[
                // On liste les fichiers services à compiler
                format!("{}/profile/v1/profile.proto", proto_root),
                format!("{}/profile/v1/profile_query.proto", proto_root),
                // Attention : vérifie si user_location.proto est dans 'profile' ou 'location'
                format!("{}/profile/v1/user_location.proto", proto_root),
            ],
            // 2. On passe la racine 'proto' comme dossier d'inclusion
            // C'est ce qui permet aux "import profile/v1/..." de fonctionner
            &[proto_root]
        )?;

    println!("cargo:rerun-if-changed={}", proto_root);

    Ok(())
}