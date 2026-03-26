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