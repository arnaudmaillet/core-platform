-- =========================================================================
-- ARCHITECTURE DE STOCKAGE DU SERVICE "POST" (PROJET WYNN)
-- Optimisée pour ScyllaDB / Hyperscale (Design 100% Dé-normalisé)
-- =========================================================================
-- =========================================================================
-- 1. DÉFINITION DE L'UDT (User Defined Type)
-- Découple proprement la structure d'un média sans décalage d'index.
-- =========================================================================
CREATE TYPE IF NOT EXISTS cql_media_asset (
    media_id uuid,
    url text,                  -- URL de stockage (CDN) de la vidéo ou de la image
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
    region text,
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
    hashtags set<text>,            -- Set natif pour indexer les tags du post (#rust, #devops... -> [rust, devops,...])
    mentions set<uuid>,            -- Liste unique des profiles notifiés/mentionnés (@alex, @lucie...  -> [019e5cef, 2e9c89a8,...])
    is_edited boolean,             -- Flag indiquant si la caption a été modifiée
    updated_at timestamp,          -- Date de la dernière modification
    dynamic_metadata text,         -- Chaîne JSON (Serde) de secours pour les futures features clients
    PRIMARY KEY ((region), author_id, post_id)
) WITH CLUSTERING ORDER BY (author_id ASC, post_id DESC)
AND comment = 'Flux de profil utilisateur optimisé pour l''hyperscale (UUIDv7 + UDT)';

-- =========================================================================
-- 3. TABLE DE LOOKUP : Accès direct (Lien partagé, Router GraphQL, Modération)
-- Optimisée pour : SELECT * WHERE post_id = ...
-- Structure 100% identique pour éviter les requêtes multi-tables (Cross-Node Scans).
-- =========================================================================
CREATE TABLE IF NOT EXISTS posts_by_id (
    region text,
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
    is_edited boolean,
    updated_at timestamp,
    dynamic_metadata text,
    PRIMARY KEY ((region), post_id)
) WITH comment = 'Lookup direct par Post ID (Copie dénormalisée pour l''API Gateway)';

ALTER TABLE posts_by_id WITH cdc = {'enabled': true};
ALTER TABLE posts_by_author WITH cdc = {'enabled': true};