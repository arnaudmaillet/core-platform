# docs/workspace.dsl

workspace "Core-platform" "Social Network - Full Scale Production Architecture" {

    model {
        user = person "User" "Utilisateur final accédant aux services via Mobile ou Web." "User"

        # --- INFRASTRUCTURE & EXTERNAL SYSTEMS ---
        !include architecture/external_systems.dsl

        backend = softwareSystem "Backend Platform" "Écosystème de microservices haute performance (Rust/Axum)." {
            !include architecture/containers.dsl
        }

        # --- RELATIONS STATIQUES ---
        !include architecture/relations.dsl
    }

    views {
        !include architecture/views.dsl
        !include flows/dynamic_flows.dsl
        
        # --- STYLES ---
        !include styles/styles.dsl
    }

    configuration {
        scope softwareSystem
    }
}