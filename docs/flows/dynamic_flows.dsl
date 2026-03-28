# docs/flows/dynamics_flows.dsl

dynamic backend "010_Keycloak_Login_Flow" "Authentification via Keycloak (OIDC)" {
    user -> keycloak "1. Login/MFA"
    keycloak -> user "2. Authorization Code"
    user -> apiBff "3. Exchange Code"
    apiBff -> keycloak "4. Backchannel Exchange"
    keycloak -> apiBff "5. Returns JWT"
    apiBff -> accountService "6. EnsureUserExists"
    apiBff -> user "7. Session Active"
}

dynamic backend "010_Keycloak_Refresh_Flow_With_Ban_Check" "Refresh sécurisé avec vérification de bannissement" {
    user -> apiBff "1. Token expiré, Refresh Request"
    apiBff -> moderationService "2. IsUserBanned"
    moderationService -> moderationDb "3. Check sanctions"
    apiBff -> keycloak "4. Request New Token"
    keycloak -> apiBff "5. Returns New JWT"
    apiBff -> user "6. Session prolongée"
}

dynamic backend "010_Account_Profile_Saga" "Cycle de vie : Création de compte et profil" {
    user -> apiBff "1. Register"
    apiBff -> accountService "2. Create Account"
    accountService -> accountDb "3. Persist"
    accountService -> kafka "4. AccountCreated Event"
    kafka -> profileService "5. Auto-create profile"
    profileService -> profileDb "6. Persist"
}

dynamic backend "010_User_Avatar_Lifecycle" "Mise à jour d'avatar et modération" {
    user -> objectStorage "1. Upload direct"
    user -> apiBff "2. Confirm Upload"
    apiBff -> profileService "3. UpdateAvatar"
    profileService -> kafka "4. AvatarChanged Event"
    kafka -> mediaWorker "5. Resize"
    mediaWorker -> aiService "6. Scan NSFW"
    aiService -> moderationService "7. Analysis"
    moderationService -> profileService "8. Revert if offensive"
}

dynamic backend "011_Social_Graph_Follow" "Flux de follow et propagation asynchrone" {
    user -> apiBff "1. Click Follow"
    apiBff -> graphService "2. Request Follow"
    graphService -> graphDb "3. Persist Relation"
    graphService -> redisGraph "4. Update Cache"
    graphService -> kafka "5. UserFollowed Event"
    kafka -> profileService "6. Increment Counter"
    kafka -> nebulaGraph "7. Update Social Graph"
}

dynamic backend "011_Friend_Recommendation_Engine" "Flux de recommandation haute performance" {
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

dynamic backend "011_Presence_Update_Flow" "Alimentation du compteur de viewers (Heartbeat)" {
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

dynamic backend "020_Post_Read_Flow" "Lecture d'un post avec hydratation distribuée et commentaires" {
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

dynamic backend "020_Create_Post_Choreography" "Cycle de vie du contenu (Event-Driven)" {
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

dynamic backend "021_Geo_Map_Flow" "Cycle de vie des Pins intelligents sur la carte" {
    user -> apiBff "1. Open Map (Viewport)"
    apiBff -> searchService "2. GetGeoPins"
    searchService -> elasticsearch "3. Geo_Tile_Grid Aggregation"
    elasticsearch -> searchService "4. Top Posts per Tile"
    searchService -> apiBff "5. Return Clusters"
    apiBff -> postService "6. Batch Get Thumbnails"
}

dynamic backend "021_Feed_Push_Fanout" "Propagation asynchrone (Fan-out)" {
    postService -> kafka "1. [EVENT] PostCreated {author_id, is_celebrity: false}"
    kafka -> feedService "2. [CONSUME] Post Event"
    feedService -> feedService "3. Check: is_celebrity == false"
    feedService -> graphService "4. [gRPC] Get Followers IDs"
    feedService -> feedCache "5. [LPUSH] Push post_id to follower timelines (Fan-out)"
}

dynamic backend "021_Feed_Read_Engine" "Lecture du Feed : Modèle Hybride & Auto-Reconstruction" {
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


dynamic backend "021_Live_Comment_Flow" "Flux Live : Diffusion instantanée et Modération asynchrone" {
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

dynamic backend "030_User_Report_Flow" "Signalement et sanction automatique" {
    user -> apiBff "1. Report Post"
    apiBff -> moderationService "2. CreateReport"
    moderationService -> moderationDb "3. SQL Persist"
    moderationService -> aiService "4. Content Analysis"
    moderationService -> kafka "5. ContentBanned Event"
    kafka -> postService "6. Mask Post"
    kafka -> searchWorker "7. Remove from ES"
}

dynamic backend "040_Notification_Massive_Fanout" "Envoi massif et dédoublonnage" {
    kafka -> notificationService "1. Engagement Event"
    notificationService -> redisNotification "2. Idempotency Check"
    notificationService -> notificationDb "3. Get Tokens"
    notificationService -> kafka "4. Produit PushTask"
    kafka -> notificationWorker "5. Execute"
    notificationWorker -> fcm "6. Send Push"
    notificationWorker -> redisNotification "7. Mark Task Done"
}

dynamic backend "050_Analytics_Ingestion_Flow" "Flux de collecte Big Data" {
    user -> analyticsCollector "1. Tracking Beacon"
    analyticsCollector -> kafka "2. Raw Events"
    kafka -> analyticsWorker "3. Enrichment"
    analyticsWorker -> clickhouse "4. Insert OLAP"
    analyticsWorker -> dataLake "5. Archive"
    apiBff -> clickhouse "6. Query Stats"
}