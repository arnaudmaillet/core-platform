-- =========================================================================
-- ARCHITECTURE DE STOCKAGE RÉGIONALE DU SERVICE "POST"
-- Optimisée pour ScyllaDB / Hyperscale (Design 100% Dé-normalisé)
-- =========================================================================

-- 💡 Le Keyspace régional sera créé et configuré dynamiquement par l'Orchestrateur en Rust.
-- Les structures ci-dessous s'appliqueront dans le keyspace courant (via la commande USE).

-- =========================================================================
-- 1. DÉFINITION DE L'UDT (User Defined Type)
-- Découple proprement la structure d'un média sans décalage d'index.
-- =========================================================================
CREATE TYPE IF NOT EXISTS cql_media_asset (
    media_id uuid,
    url text,                  -- URL de stockage (CDN) de la vidéo ou de l'image
    thumbnail_url text,        -- URL de la miniature associée
    duration_seconds int,      -- Durée en secondes (0 pour une image, <= 10 pour vidéo carrousel)
    width int,                 -- Largeur en pixels
    height int,                -- Hauteur en pixels
    media_type text,           -- 'video' ou 'image' (Utile pour le lecteur de l'app mobile)
    mime_type text             -- 'video/mp4', 'image/webp', etc.
);

-- =========================================================================
-- 2. TABLE PRINCIPALE : Flux de profil (Toutes les publications d'un auteur)
-- Optimisée pour : SELECT * WHERE author_id = ...
-- Tri chronologique natif décroissant assuré par l'UUIDv7 du post_id.
-- =========================================================================
CREATE TABLE IF NOT EXISTS posts_by_author (
    author_id uuid,
    post_id uuid,                  -- UUIDv7 (Timestamp 48-bit + 74-bit entropie aléatoire)
    post_type text,                -- 'video', 'carousel', 'image', 'text' (Format global du post)
    caption text,                  -- Texte du post ou description de la vidéo
    
    -- Liste ordonnée de nos médias imbriqués (Zéro octet utilisé si post_type = 'text')
    media_list list<frozen<cql_media_asset>>, 
    
    total_duration_seconds int,    -- Somme des durées des vidéos (Pratique pour l'algorithme)
    allowed_comment_hands boolean, -- Paramètre d'interaction (Activer/Désactiver les commentaires)
    visibility_level text,         -- Gestion de la confidentialité : 'public', 'friends', 'private'
    music_id uuid,                 -- Optionnel : ID de la piste audio associée au post
    hashtags set<text>,            -- Set natif pour indexer les tags du post (#rust, #devops)
    mentions set<uuid>,            -- Liste unique des profiles notifiés/mentionnés (@alex, @lucie)
    
    -- MÉTADONNÉES DE TEMPS MÉTIER
    -- Concurrence & Conflits réseau résolus via Client-Side Timestamps (USING TIMESTAMP applicatif)
    edited_at timestamp,           -- Date de la dernière modification du contenu (Métier)
    created_at timestamp,          -- Date de création du post
    
    dynamic_metadata text,         -- Chaîne JSON (Serde) de secours pour les futures features clients
    PRIMARY KEY (author_id, post_id)
) WITH CLUSTERING ORDER BY (post_id DESC)
AND comment = 'Flux de profil utilisateur optimisé pour l''hyperscale (UUIDv7 + Clustering Key DESC - LWW Timestamp Engine)';

-- =========================================================================
-- 3. TABLE DE LOOKUP : Accès direct (Lien partagé, Router GraphQL, Modération)
-- Optimisée pour : SELECT * WHERE post_id = ...
-- Structure 100% dénormalisée pour éviter les requêtes multi-tables (Cross-Node Scans).
-- =========================================================================
CREATE TABLE IF NOT EXISTS posts_by_id (
    post_id uuid,
    author_id uuid,
    post_type text,
    caption text,
    
    media_list list<frozen<cql_media_asset>>, 
    
    total_duration_seconds int,
    allowed_comment_hands boolean,
    visibility_level text,
    music_id uuid,
    hashtags set<text>,
    mentions set<uuid>,
    
    -- MÉTADONNÉES DE TEMPS MÉTIER
    edited_at timestamp,
    created_at timestamp,
    
    dynamic_metadata text,
    PRIMARY KEY (post_id)
) WITH comment = 'Lookup direct par Post ID (Copie dénormalisée pour l''API Gateway - LWW Timestamp Engine)';

-- =========================================================================
-- 4. ACTIVATION DU CDC (Change Data Capture) D'INFRASTRUCTURE
-- C'est ce bloc qui permet de propager les événements vers Kafka de manière asynchrone,
-- libérant le code applicatif Rust de toute gestion de table Outbox.
-- =========================================================================
ALTER TABLE posts_by_author WITH cdc = {'enabled': true};
ALTER TABLE posts_by_id WITH cdc = {'enabled': true};