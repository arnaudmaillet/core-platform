workspace "Core-platform" "Social Network - Full Scale Production Architecture" {

    model {
        user = person "User" "Utilisateur final accédant aux services via Mobile ou Web." "User"

        # --- INFRASTRUCTURE & EXTERNAL SYSTEMS ---
        fcm = softwareSystem "FCM / APNS" "Push Notification Gateways (Google/Apple)." "External"
        emailProvider = softwareSystem "Email Provider" "Service d'envoi d'emails transactionnels (SendGrid/Postmark)." "External"
        objectStorage = softwareSystem "S3 / MinIO" "Stockage d'objets immuables (Images/Vidéos)." "Infrastructure"
        cdn = softwareSystem "CDN" "Edge Content Delivery Network pour le caching des médias." "Infrastructure"
        keycloak = softwareSystem "Keycloak (IAM)" "Gestionnaire d'identité, OAuth2/OIDC et SSO." "Infrastructure"
        aiService = softwareSystem "AI Moderation API" "Analyse automatisée (AWS Rekognition / Google Vision)." "External"

        backend = softwareSystem "Backend Platform" "Écosystème de microservices haute performance (Rust/Axum)." {
            
            # --- 0. SHARED BACKBONE ---
            kafka = container "Kafka Cluster" "Bus d'événements distribué pour la communication asynchrone." "Apache Kafka" "MessageBroker"
            
            # --- 1. EDGE LAYER ---
            group "Edge Layer" {
                apiBff = container "API BFF" "Agrégateur pour le trafic HTTP standard." "Rust/Axum" "BFF"
                liveBff = container "Real-time BFF" "Passerelle WebSockets pour le contenu Live." "Rust/Axum/Tokio" "BFF"
                redisBff = container "Redis (BFF)" "Cache de sessions et agrégation d'objets JSON." "Redis" "Cache"
            }

            # --- 2. IDENTITY DOMAIN ---
            group "Identity Domain" {
                accountService = container "Account Service" "Gestion du cycle de vie des comptes et synchronisation IAM." "Rust/Axum" "Service"
                accountDb = container "PostgreSQL (Account)" "Données de profil utilisateur et métadonnées de compte." "PostgreSQL" "Database"
                redisUser = container "Redis (User)" "Cache de sessions et états de connexion." "Redis" "Cache"
            }

            # --- 3. PROFILE DOMAIN ---
            group "Profile Domain" {
                profileService = container "Profile Service" "Gestion des personas et métadonnées publiques." "Rust/Axum" "Service"
                profileDb = container "ScyllaDB (Profile)" "Stockage hautement disponible des attributs de profil." "ScyllaDB" "Database"
                redisProfile = container "Redis (Profile)" "Cache distribué pour les entités profils (L1)." "Redis" "Cache"
            }

            # --- 4. SOCIAL GRAPH DOMAIN ---
            group "Social Graph Domain" {
                graphService = container "Social Graph Service" "Gestion des relations (Followers/Following)." "Rust/Axum" "Service"
                graphDb = container "ScyllaDB (Graph)" "Stockage des relations orienté colonnes." "ScyllaDB" "Database"
                nebulaGraph = container "NebulaGraph" "Moteur de graphe pour les recommandations complexes." "NebulaGraph" "Database"
                redisGraph = container "Redis (Graph)" "Cache des relations chaudes." "Redis" "Cache"
            }

            # --- 5. DISCOVERY & SEARCH DOMAIN ---
            group "Search & Discovery Domain" {
                searchService = container "Search Service" "Abstraction des recherches géo-spatiales et textuelles." "Rust/Axum" "Service"
                searchWorker = container "Search Worker" "Indexation asynchrone des contenus." "Rust/Tokio" "Worker"
                elasticsearch = container "Elasticsearch" "Moteur de recherche et d'indexation géo-spatiale." "Elasticsearch" "SearchEngine"
                recommendationService = container "Recommendation Service" "Orchestrateur de scoring et de ranking." "Rust/Axum" "Service"
                recoCache = container "Redis (Recommendation)" "Cache des suggestions pré-calculées." "Redis" "Cache"
            }

            # --- 6. CONTENT DOMAIN (POST) ---
            group "Post Domain" {
                postService = container "Post Service" "Gestion du cycle de vie des contenus (Posts)." "Rust/Axum" "Service"
                postDb = container "ScyllaDB (Post)" "Stockage distribué des publications." "ScyllaDB" "Database"
                redisPost = container "Redis (Post)" "Cache d'entités de contenu (L2)." "Redis" "Cache"
            }

            # --- 6.1 COMMENT DOMAIN ---
            # group "Comment Domain" {
            #     commentService = container "Comment Service" "Gestion des fils de discussion et réponses." "Rust/Axum" "Service"
            #     commentDb = container "ScyllaDB (Comment)" "Stockage des hiérarchies de commentaires." "ScyllaDB" "Database"
            #     redisComment = container "Redis (Comment)" "Cache des commentaires chauds (Top Level)." "Redis" "Cache"
            # }

            group "Comment Domain" {
                commentService = container "Comment Service" "Gestion des fils de discussion et réponses." "Rust/Axum" "Service"
                commentDb = container "ScyllaDB (Comment)" "Stockage des hiérarchies de commentaires." "ScyllaDB" "Database"
                redisComment = container "Redis (Comment)" "Cache des commentaires chauds." "Redis" "Cache"
                redisPubSub = container "Redis (Pub/Sub)" "Bus de messages éphémères pour le Live." "Redis" "Cache"
            }

            # --- 7. ENGAGEMENT DOMAIN ---
            group "Engagement Domain" {
                engagementService = container "Engagement Service" "Gestion des likes, votes et compteurs temps-réel." "Rust/Axum" "Service"
                engagementDb = container "ScyllaDB (Engagement)" "Historique des interactions (Time-series)." "ScyllaDB" "Database"
                redisEngagement = container "Redis (Engagement)" "Compteurs atomiques haute fréquence." "Redis" "Cache"
            }

            # --- 8. FEED DOMAIN ---
            group "Feed Domain" {
                feedService = container "Feed Service" "Générateur de timelines personnalisées." "Rust/Axum" "Service"
                feedCache = container "Redis (Feed)" "Stockage des timelines pré-calculées (Fan-out)." "Redis" "Cache"
            }

            # --- 9. MEDIA DOMAIN ---
            group "Media Domain" {
                mediaService = container "Media Service" "Gestionnaire des métadonnées et accès médias." "Rust/Axum" "Service"
                mediaWorker = container "Media Worker" "Traitement, transcodage et compression FFmpeg." "Rust/Tokio" "Worker"
            }

            # --- 10. MODERATION DOMAIN ---
            group "Moderation Domain" {
                moderationService = container "Moderation Service" "Gestion des signalements, bans et sanctions." "Rust/Axum" "Service"
                moderationDb = container "PostgreSQL (Moderation)" "Audit logs et files de modération." "PostgreSQL" "Database"
            }

            # --- 11. NOTIFICATION DOMAIN ---
            group "Notification Domain" {
                notificationService = container "Notification Service" "Orchestration des notifications multi-canal." "Rust/Axum" "Service"
                notificationDb = container "PostgreSQL (Notification)" "Préférences et tokens terminaux." "PostgreSQL" "Database"
                notificationWorker = container "Notification Worker" "Exécution des envois (FCM/SMTP)." "Rust/Tokio" "Worker"
                redisNotification = container "Redis (Notification)" "Idempotence et Rate Limiting." "Redis" "Cache"
            }

            # --- 12. ANALYTICS DOMAIN ---
            group "Analytics Domain" {
                analyticsCollector = container "Analytics Collector" "Ingestion de télémétrie (Beacons)." "Rust/Axum" "Service"
                analyticsWorker = container "Analytics Worker" "Traitement ETL et agrégation de flux." "Rust/Tokio" "Worker"
                clickhouse = container "ClickHouse" "Base OLAP haute performance pour analytics." "ClickHouse" "Database"
                dataLake = container "Data Lake" "Archivage long terme (Parquet/Iceberg)." "S3" "Database"
            }

            # --- RELATIONS STATIQUES (MODÈLE) ---
            user -> apiBff "Requêtes API" "HTTPS/JSON"
            user -> liveBff "WebSocket pour contenu Live" "WSS/JSON"
            user -> keycloak "Authentification" "HTTPS"
            user -> objectStorage "Upload via Presigned URL" "HTTPS/S3"
            user -> cdn "Consomme les médias" "HTTPS"
            user -> analyticsCollector "Émet des événements" "HTTPS/JSON"

            apiBff -> redisBff "Mise en cache" "RESP"
            apiBff -> keycloak "Validation JWKS" "HTTPS"
            apiBff -> accountService "Profil privé" "gRPC/Protobuf"
            apiBff -> profileService "Profil public" "gRPC/Protobuf"
            apiBff -> postService "Contenu" "gRPC/Protobuf"
            apiBff -> commentService "Gestion des commentaires" "gRPC/Protobuf"
            apiBff -> searchService "Recherche & Géo" "gRPC/Protobuf"
            apiBff -> recommendationService "Discovery" "gRPC/Protobuf"
            apiBff -> engagementService "Compteurs" "gRPC/Protobuf"
            apiBff -> graphService "Relations" "gRPC/Protobuf"
            apiBff -> feedService "Timelines" "gRPC/Protobuf"
            apiBff -> mediaService "Uploads" "gRPC/Protobuf"
            apiBff -> moderationService "Signalements" "gRPC/Protobuf"
            apiBff -> clickhouse "Lecture statistiques" "HTTPS"

            liveBff -> redisPubSub "Souscrit aux événements" "RESP"
            liveBff -> commentService "Envoie les nouveaux commentaires en direct" "gRPC/Protobuf"
            liveBff -> engagementService "Signale la présence et les heartbeats des utilisateurs" "gRPC/Protobuf"

            accountService -> accountDb "Lecture/Écriture" "PostgreSQL Protocol"
            accountService -> redisUser "Sessions" "RESP"
            accountService -> kafka "Émet [Account_Created]" "Kafka Protocol"
            
            profileService -> profileDb "Persistance" "CQL"
            profileService -> redisProfile "Cache L1" "RESP"
            profileService -> kafka "Émet [Profile_Updated]" "Kafka Protocol"
            kafka -> profileService "Consomme [User_Followed/Avatar_Changed]" "Kafka Protocol"
            
            graphService -> graphDb "Persistance" "CQL"
            graphService -> nebulaGraph "Graph Queries" "Thrift"
            graphService -> redisGraph "Cache L1" "RESP"
            graphService -> kafka "Émet [User_Followed]" "Kafka Protocol"
            kafka -> nebulaGraph "Sync des relations" "Kafka Protocol"

            searchService -> elasticsearch "Requêtes" "HTTPS/JSON"
            searchWorker -> elasticsearch "Indexation" "HTTPS/JSON"
            kafka -> searchWorker "Consomme [Post_Created/Content_Banned]" "Kafka Protocol"

            recommendationService -> recoCache "Cache suggestions" "RESP"
            recommendationService -> graphService "Extraction candidats" "gRPC/Protobuf"
            recommendationService -> searchService "Filtres géo" "gRPC/Protobuf"
            recommendationService -> profileService "Hydratation" "gRPC/Protobuf"
            
            postService -> postDb "Persistance" "CQL"
            postService -> redisPost "Cache L2" "RESP"
            postService -> kafka "Émet [Post_Created]" "Kafka Protocol"
            kafka -> postService "Update statut post" "Kafka Protocol"

            commentService -> commentDb "Persistance" "CQL"
            commentService -> redisComment "Cache L1" "RESP"
            commentService -> redisPubSub "Publie les nouveaux commentaires en temps-réel" "RESP / PubSub"
            commentService -> kafka "Émet [Comment_Created]" "Kafka Protocol"
            redisPubSub -> liveBff "Diffuse les messages aux instances connectées" "RESP / PubSub"
            kafka -> commentService "Consomme [Content_Banned]" "Kafka Protocol"
            kafka -> notificationService "Consomme [Comment_Created]" "Kafka Protocol"

            feedService -> feedCache "Stockage Timelines" "RESP"
            feedService -> graphService "Get Following" "gRPC/Protobuf"
            feedService -> postService "Get Posts" "gRPC/Protobuf"
            feedService -> profileService "Get Authors" "gRPC/Protobuf"
            feedService -> feedService "Logique interne (Filtrage, Fusion, Tri)" "In-Memory"
            kafka -> feedService "Déclenche Fan-out" "Kafka Protocol"
            
            engagementService -> redisEngagement "Compteurs atomiques" "RESP"
            engagementService -> engagementDb "Persistence" "CQL"
            engagementService -> kafka "Émet [Like_Added]" "Kafka Protocol"
            engagementService -> redisPubSub "Publie les mises à jour des compteurs (Likes, Viewers) en temps réel" "RESP / PubSub"

            mediaService -> kafka "Émet [Media_Uploaded]" "Kafka Protocol"
            mediaWorker -> objectStorage "Lecture/Écriture" "S3 Protocol"
            mediaWorker -> aiService "Analyse IA" "HTTPS/JSON"
            mediaWorker -> kafka "Émet [Media_Processed]" "Kafka Protocol"
            kafka -> mediaWorker "Consomme [Media_Uploaded/Avatar_Changed]" "Kafka Protocol"

            moderationService -> moderationDb "Persistance" "PostgreSQL Protocol"
            moderationService -> aiService "Scan automatique" "HTTPS/JSON"
            moderationService -> kafka "Émet [Content_Banned/Approved]" "Kafka Protocol"
            moderationService -> accountService "Lockdown" "gRPC/Protobuf"
            moderationService -> profileService "Reset Avatar" "gRPC/Protobuf"
            moderationService -> notificationService "Alerte modération" "gRPC/Protobuf"
            
            notificationService -> notificationDb "Persistence" "PostgreSQL Protocol"
            notificationService -> graphService "Get Followers" "gRPC/Protobuf"
            notificationService -> kafka "Produit [Push_Task]" "Kafka Protocol"
            notificationService -> redisNotification "Vérifie Idempotence" "RESP"
            
            notificationWorker -> kafka "Consomme [Push_Task]" "Kafka Protocol"
            notificationWorker -> fcm "Push" "HTTPS"
            notificationWorker -> emailProvider "Email" "HTTPS"
            notificationWorker -> redisNotification "Marque comme envoyé" "RESP"

            analyticsCollector -> kafka "Flux brut" "Kafka Protocol"
            analyticsWorker -> kafka "Consomme flux" "Kafka Protocol"
            analyticsWorker -> clickhouse "Bulk Insert" "Native"
            analyticsWorker -> dataLake "Archivage" "S3 Protocol"
        }
    }

    views {
        systemContext backend "01_System_Context" {
            include *
            autolayout lr
        }

        container backend "02_Architecture_Overview" {
            include *
            autolayout lr
        }

        dynamic backend "Keycloak_Login_Flow" "Authentification via Keycloak (OIDC)" {
            user -> keycloak "1. Login/MFA"
            keycloak -> user "2. Authorization Code"
            user -> apiBff "3. Exchange Code"
            apiBff -> keycloak "4. Backchannel Exchange"
            keycloak -> apiBff "5. Returns JWT"
            apiBff -> accountService "6. EnsureUserExists"
            apiBff -> user "7. Session Active"
        }

        dynamic backend "Keycloak_Refresh_Flow_With_Ban_Check" "Refresh sécurisé avec vérification de bannissement" {
            user -> apiBff "1. Token expiré, Refresh Request"
            apiBff -> moderationService "2. IsUserBanned"
            moderationService -> moderationDb "3. Check sanctions"
            apiBff -> keycloak "4. Request New Token"
            keycloak -> apiBff "5. Returns New JWT"
            apiBff -> user "6. Session prolongée"
        }

        dynamic backend "Account_Profile_Saga" "Cycle de vie : Création de compte et profil" {
            user -> apiBff "1. Register"
            apiBff -> accountService "2. Create Account"
            accountService -> accountDb "3. Persist"
            accountService -> kafka "4. AccountCreated Event"
            kafka -> profileService "5. Auto-create profile"
            profileService -> profileDb "6. Persist"
        }

        dynamic backend "Post_Read_Flow" "Lecture d'un post avec hydratation distribuée et commentaires" {
            user -> apiBff "1. Request Post Detail (ID: post_123)"
            apiBff -> redisBff "2. [READ] Check Global Aggregated Cache"

            # --- PHASE 1 : RÉCUPÉRATION DU CONTENU ---
            apiBff -> postService "3. [gRPC] Get Post Content"
            postService -> redisPost "4. [READ] Check Local Post Cache"
            
            # --- PHASE 2 : RÉCUPÉRATION DE L'ENGAGEMENT (LIKES) ---
            apiBff -> engagementService "5. [gRPC] Get Live Counters (Post + Authors + Viewers)"
            engagementService -> redisEngagement "6. [READ] Get Hot Counters"
            
            # --- PHASE 3 : RÉCUPÉRATION DES COMMENTAIRES (NOUVEAU) ---
            apiBff -> commentService "7. [gRPC] Get Top Comments (Limit 5, Sorted by Relevance)"
            commentService -> redisComment "8. [READ] Check Hot Thread Cache"
            
            # --- PHASE 4 : HYDRATATION DES AUTEURS ---
            # Le BFF extrait tous les IDs d'auteurs (Post + Commentaires)
            apiBff -> profileService "9. [gRPC BatchGet] Hydrate All Unique Profiles"
            profileService -> redisProfile "10. [READ] Check Profile Cache"
            
            # --- PHASE 5 : FINALISATION ---
            apiBff -> redisBff "11. [WRITE] Cache Aggregated JSON (TTL 30s)"
            apiBff -> user "12. Return Full View (Contenu + Likes + Viewers + Coms)"
        }

        dynamic backend "Social_Graph_Follow" "Flux de follow et propagation asynchrone" {
            user -> apiBff "1. Click Follow"
            apiBff -> graphService "2. Request Follow"
            graphService -> graphDb "3. Persist Relation"
            graphService -> redisGraph "4. Update Cache"
            graphService -> kafka "5. UserFollowed Event"
            kafka -> profileService "6. Increment Counter"
            kafka -> nebulaGraph "7. Update Social Graph"
        }

        dynamic backend "Friend_Recommendation_Engine" "Flux de recommandation haute performance" {
            user -> apiBff "1. Request Suggestions"
            apiBff -> recommendationService "2. GetRecommendations"
            recommendationService -> recoCache "3. Check pre-calculated results"
            recommendationService -> graphService "4. Get Candidates"
            graphService -> nebulaGraph "5. Find k-hop neighbors"
            recommendationService -> searchService "6. Filter by Geo/Interests"
            searchService -> elasticsearch "7. Query Scoring"
            recommendationService -> profileService "8. Batch Get Profiles"
            recommendationService -> recoCache "9. Cache results"
        }

        dynamic backend "Geo_Map_Flow" "Cycle de vie des Pins intelligents sur la carte" {
            user -> apiBff "1. Open Map (Viewport)"
            apiBff -> searchService "2. GetGeoPins"
            searchService -> elasticsearch "3. Geo_Tile_Grid Aggregation"
            elasticsearch -> searchService "4. Top Posts per Tile"
            searchService -> apiBff "5. Return Clusters"
            apiBff -> postService "6. Batch Get Thumbnails"
        }

        dynamic backend "Feed_Push_Fanout" "Propagation asynchrone (Fan-out)" {
            postService -> kafka "1. [EVENT] PostCreated {author_id, is_celebrity: false}"
            kafka -> feedService "2. [CONSUME] Post Event"
            feedService -> feedService "3. Check: is_celebrity == false"
            feedService -> graphService "4. [gRPC] Get Followers IDs"
            feedService -> feedCache "5. [LPUSH] Push post_id to follower timelines (Fan-out)"
        }

        dynamic backend "Feed_Read_Engine" "Lecture du Feed : Modèle Hybride & Auto-Reconstruction" {
            user -> apiBff "1. Demande le Fil d'actualité (Page 1)"
            apiBff -> feedService "2. [gRPC] GetFeed(user_id, page_size)"
            
            # --- PHASE 1 : RÉCUPÉRATION DU FLUX STANDARD (PUSH) ---
            feedService -> feedCache "3. [LRANGE] Récupère les IDs pré-calculés (Timeline des amis 'normaux')"
            
            # --- SOUS-FLUX : CACHE MISS / WARM-UP ---
            # Si l'utilisateur est inactif, on reconstruit sa timeline à la volée
            feedService -> graphService "4. [IF_CACHE_MISS] Récupère la liste complète des suivis"
            feedService -> postService "5. [IF_CACHE_MISS] Pull des derniers posts (filtrés hors célébrités)"
            feedService -> feedCache "6. [IF_CACHE_MISS] Remplit le cache Redis (LPUSH)"
            
            # --- PHASE 2 : RÉCUPÉRATION DES CÉLÉBRITÉS (PULL / FAN-IN) ---
            # Optimisation : on utilise la liste des IDs suivis marqués 'is_celebrity' déjà en cache
            feedService -> profileService "7. [L1 Cache] Récupère les IDs des célébrités suivies par l'utilisateur"
            feedService -> postService "8. [PULL gRPC] Récupère les N derniers posts pour ces IDs spécifiques"
            
            # --- PHASE 3 : FUSION ET HYDRATATION ---
            feedService -> feedService "9. Fusionne Push + Pull, dédoublonne et trie chronologiquement"
            feedService -> postService "10. [gRPC Batch] Hydrate le contenu des posts"
            feedService -> profileService "11. [gRPC Batch] Hydrate les auteurs (Avatar, Badge VIP)"
            
            feedService -> apiBff "12. Retourne le Feed hybride agrégé"
            apiBff -> user "13. Rendu de la Timeline"
        }

        dynamic backend "Presence_Update_Flow" "Alimentation du compteur de viewers (Heartbeat)" {
            # --- ÉTAPE 1 : CONNEXION ---
            user -> liveBff "1. Ouvre WebSocket (post_123)"
            
            # --- ÉTAPE 2 : SIGNAL D'ENTRÉE ---
            liveBff -> engagementService "2. [gRPC] NotifyPresence(user_id, post_123)"
            engagementService -> redisEngagement "3. [PFADD] presence:post_123 (HyperLogLog)"
            
            # --- ÉTAPE 3 : MAINTIEN (HEARTBEAT) ---
            # Le mobile envoie un "ping" toutes les 20s pour rester dans le compteur
            user -> liveBff "4. [WS] Keep-alive / Ping"
            liveBff -> engagementService "5. [gRPC] TouchPresence(user_id, post_123)"
            engagementService -> redisEngagement "6. [PEXPIRE] Reset TTL sur la présence"
            
            # --- ÉTAPE 4 : DIFFUSION DU NOUVEAU COMPTE ---
            # Si le nombre change significativement, on prévient tout le monde
            engagementService -> redisPubSub "7. [PUBLISH] channel:post_123 {type: 'VIEWER_COUNT', count: 4250}"
            redisPubSub -> liveBff "8. [SUBSCRIBE]"
            liveBff -> user "9. [WS PUSH] Mise à jour du compteur sur l'écran"
        }

        dynamic backend "Live_Comment_Flow" "Flux Live : Diffusion instantanée et Modération asynchrone" {
            # --- PHASE 1 : RÉCEPTION & PERSISTANCE ---
            user -> liveBff "1. Envoie un message (WebSocket)"
            liveBff -> commentService "2. [gRPC] CreateLiveComment"
            commentService -> commentDb "3. [WRITE] ScyllaDB (Status: LIVE)"
            
            # --- PHASE 2 : DIFFUSION INSTANTANÉE (CHEMIN COURT) ---
            # Le message s'affiche sur tous les écrans en <100ms
            commentService -> redisPubSub "4. [PUBLISH] channel:post_123"
            redisPubSub -> liveBff "5. [SUBSCRIBE]"
            liveBff -> user "6. [WS PUSH] Message affiché (Optimistic)"
            
            # --- PHASE 3 : PIPELINE DE FIABILITÉ (CHEMIN LONG) ---
            commentService -> kafka "7. [EVENT] CommentCreated"
            
            # La modération travaille en arrière-plan
            kafka -> moderationService "8. [CONSUME] Analyse NLP / Toxicité"
            moderationService -> aiService "9. Scan IA"
            
            # --- PHASE 4 : RÉACTIONS (CORRECTION SI BESOIN) ---
            # SCÉNARIO : LE MESSAGE EST TOXIQUE
            moderationService -> kafka "10. [EVENT] ContentBanned"
            
            # 1. Le Comment Service le supprime en DB
            kafka -> commentService "11. [CONSUME] Delete / Hide in DB"
            
            # 2. On envoie un signal de suppression en Temps Réel !
            commentService -> redisPubSub "12. [PUBLISH] channel:post_123 {type: 'DELETE', id: 'msg_99'}"
            redisPubSub -> liveBff "13. [WS PUSH] L'UI retire le message de l'écran"
            
            # SCÉNARIO : LE MESSAGE EST OK
            # On déclenche les services "lents" (Search, Engagement, Notification)
            kafka -> engagementService "14. [CONSUME] Incrémente les compteurs globaux"
            kafka -> searchWorker "15. [CONSUME] Indexation"
            kafka -> notificationService "16. [CONSUME] Notify Post Owner"
        }

        dynamic backend "Create_Post_Choreography" "Cycle de vie du contenu (Event-Driven)" {
            user -> apiBff "1. Submit Metadata"
            apiBff -> postService "2. Create (PROCESSING)"
            postService -> kafka "3. PostCreated Event"
            kafka -> mediaWorker "4. Transcode Media"
            mediaWorker -> aiService "5. Scan AI"
            aiService -> moderationService "6. Risk Scoring"
            moderationService -> kafka "7. ContentApproved Event"
            kafka -> postService "8. Status = VISIBLE"
            kafka -> feedService "9. Trigger Fan-out"
        }

        dynamic backend "User_Avatar_Lifecycle" "Mise à jour d'avatar et modération" {
            user -> objectStorage "1. Upload direct"
            user -> apiBff "2. Confirm Upload"
            apiBff -> profileService "3. UpdateAvatar"
            profileService -> kafka "4. AvatarChanged Event"
            kafka -> mediaWorker "5. Resize"
            mediaWorker -> aiService "6. Scan NSFW"
            aiService -> moderationService "7. Analysis"
            moderationService -> profileService "8. Revert if offensive"
        }

        dynamic backend "User_Report_Flow" "Signalement et sanction automatique" {
            user -> apiBff "1. Report Post"
            apiBff -> moderationService "2. CreateReport"
            moderationService -> moderationDb "3. SQL Persist"
            moderationService -> aiService "4. Content Analysis"
            moderationService -> kafka "5. ContentBanned Event"
            kafka -> postService "6. Mask Post"
            kafka -> searchWorker "7. Remove from ES"
        }

        dynamic backend "Notification_Massive_Fanout" "Envoi massif et dédoublonnage" {
            kafka -> notificationService "1. Engagement Event"
            notificationService -> redisNotification "2. Idempotency Check"
            notificationService -> notificationDb "3. Get Tokens"
            notificationService -> kafka "4. Produit PushTask"
            kafka -> notificationWorker "5. Execute"
            notificationWorker -> fcm "6. Send Push"
            notificationWorker -> redisNotification "7. Mark Task Done"
        }

        dynamic backend "Analytics_Ingestion_Flow" "Flux de collecte Big Data" {
            user -> analyticsCollector "1. Tracking Beacon"
            analyticsCollector -> kafka "2. Raw Events"
            kafka -> analyticsWorker "3. Enrichment"
            analyticsWorker -> clickhouse "4. Insert OLAP"
            analyticsWorker -> dataLake "5. Archive"
            apiBff -> clickhouse "6. Query Stats"
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
                shape Cylinder
                background #c127e8
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