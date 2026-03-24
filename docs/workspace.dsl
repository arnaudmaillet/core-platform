workspace "Core-platform" "Event-driven architecture" {

    model {
        user = person "User" "Utilisateur final" "User"
        
        backend = softwareSystem "Backend" "Social Network Infrastructure" {
            # --- Groupe Point d'Entrée ---
            group "Edge Layer" {
                bff = container "BFF Service" "Agrégateur API" "Rust/Axum" "BFF"
                redisBff = container "BFF Cache" "Cache d'agrégation" "Redis" "Cache"
            }

            # --- Groupe Domaine Users ---
            group "User Domain" {
                userService = container "User Service" "Profils & Auth" "Rust/Axum" "Service"
                userDb = container "User DB" "PostgreSQL" "PostgreSQL" "Database" 
                redisUser = container "User Cache" "Sessions" "Redis" "Cache"
            }

            # --- Groupe Domaine Posts ---
            group "Post Domain" {
                postService = container "Post Service" "Gestion des contenus" "Rust/Axum" "Service"
                redisPost = container "Post Cache" "Cache d'entités" "Redis" "Cache"
                scyllaDb = container "ScyllaDB" "Stockage Posts & UserCache" "ScyllaDB" "Database"
            }

            # --- Groupe Domaine Engagement ---
            group "Engagement Domain" {
                engagementService = container "Engagement Service" "Likes" "Rust/Axum" "Service"
                redisEngagement = container "Engagement Cache" "Counters" "Redis" "Cache"
                engagementDb = container "Engagement DB" "PostgreSQL" "PostgreSQL" "Database"
            }

            # --- Infrastructure Commune ---
            kafka = container "Kafka" "Event Bus" "Kafka" "MessageBroker"
            searchWorker = container "Search Worker" "Indexing" "Rust/Axum"
            
            # --- Relations Statiques (CORRIGÉES AVEC TECHNOLOGIES) ---
            user -> bff "Demande des données" "HTTPS"
            user -> backend "Utilise" "HTTPS"
            
            bff -> redisBff "Accède au cache L1" "Redis Protocol"
            bff -> postService "Appel gRPC" "gRPC"
            bff -> engagementService "Appel gRPC" "gRPC"
            bff -> userService "Appel gRPC" "gRPC"
            
            userService -> userDb "Lit/Écrit" "SQL"
            userService -> redisUser "Cache de session" "Redis Protocol"
            userService -> kafka "Publie des événements" "Kafka Protocol"
            
            postService -> redisPost "Accède au cache L2" "Redis Protocol"
            postService -> scyllaDb "Lit/Écrit Posts" "CQL"
            postService -> kafka "Publie des événements" "Kafka Protocol"
            
            engagementService -> redisEngagement "Compteurs atomiques" "Redis Protocol"
            engagementService -> engagementDb "Persistence" "SQL"
            engagementService -> kafka "Publie des événements" "Kafka Protocol"
            
            kafka -> postService "Synchronise UserCache" "Kafka Protocol"
            kafka -> bff "Invalide le cache" "Kafka Protocol"
            kafka -> searchWorker "Envoie pour indexation" "Kafka Protocol"
            
            searchWorker -> kafka "Consomme les flux" "Kafka Protocol"
        }
    }

    views {
        systemContext backend "SystemContext" {
            include *
            autolayout lr
        }

        container backend "01_Architecture_Overview" {
            include *
            autolayout lr
        }

        dynamic backend "Standard_Read_Pattern" "Flux de lecture multiniveau" {
            user -> bff "Demande un Post"
            bff -> redisBff "1. [READ] Check cache d'agrégation"
            bff -> postService "2a. [gRPC] Get Post + Author Data"
            bff -> engagementService "2b. [gRPC] Get Live Counters"
            postService -> redisPost "3. [READ] Check cache entité enrichie"
            postService -> scyllaDb "4. [MISS] Read 'posts' & 'user_cache'"
            engagementService -> redisEngagement "5. [READ] Check hot counters"
            engagementService -> engagementDb "6. [MISS] Read counters from PostgreSQL"
            bff -> redisBff "7. [WRITE] Fusionne les retours et met en cache"
        }

        dynamic backend "User_Profile_Sync_Saga" "Synchronisation profil vers Post Service" {
            user -> bff "Change son avatar"
            bff -> userService "1. Update Profile (gRPC)"
            userService -> userDb "2. [PERSIST] Update PostgreSQL"
            userService -> kafka "3. [NOTIFY] Event: UserUpdated"
            kafka -> postService "4. [CONSUME] Reçoit la mise à jour"
            postService -> scyllaDb "5. [UPDATE] Met à jour la table 'user_cache'"
            postService -> redisPost "6. [INVALIDATE] Supprime cache"
        }

        dynamic backend "Like_Interaction_Pattern" "Flux d'écriture optimisé compteurs" {
            user -> bff "Clique sur 'Like'"
            bff -> engagementService "1. Envoi de l'interaction (gRPC)"
            engagementService -> redisEngagement "2. [ATOMIC INCR] Incrémente en mémoire"
            engagementService -> engagementDb "3. [ASYNC WRITE] Transaction SQL"
            engagementService -> kafka "4. [EVENT] Publie 'LikeAdded'"
            kafka -> bff "5. [EVICT] Refresh cache BFF"
        }
        
        styles {
            element "Element" {
                color #ffffff
                background #2d2d2d
            }
        
            element "Person" {
                shape Person
                background #08427b
                color #ffffff
            }
            element "Container" {
                background #1168bd
            }
            element "Component" {
                background #1168bd
            }
            element "Database" {
                shape Cylinder
                background #f5da81
                color #000000
            }
            element "MessageBroker" {
                shape Pipe
                background #85bb65
            }
            element "SearchEngine" {
                shape Component
                background #feb144
            }
            element "BFF" {
                background #08427b
            }

            element "Cache" {
                background #d43b33
                color #ffffff
                shape Cylinder
            }
        }
    }

    configuration {
        scope softwareSystem
    }
}