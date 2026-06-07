-- crates/social/migrations/scylla/0001_init_social_schema.cql

-- ============================================================================
-- TABLE 1 : Les Abonnements (Following)
-- Répond à : "Qui est-ce que le profil A suit ?" et "Est-ce que A suit B ?"
-- Partitionnée par follower_id. Toutes les relations créées par un utilisateur 
-- sont localisées sur le même Shard.
-- ============================================================================
CREATE TABLE IF NOT EXISTS followings (
    follower_id uuid,
    following_id uuid,
    created_at timestamp,
    PRIMARY KEY (follower_id, following_id)
) 
WITH CLUSTERING ORDER BY (following_id DESC)
AND comment = 'Table optimisee pour lister les abonnements d un utilisateur et valider un lien';

-- ============================================================================
-- TABLE 2 : Les Abonnés (Followers)
-- Répond à : "Qui suit le profil B ?"
-- Partitionnée par following_id. Indispensable pour lister les abonnés 
-- d'une célébrité sans faire de Full Table Scan.
-- ============================================================================
CREATE TABLE IF NOT EXISTS followers (
    following_id uuid,
    follower_id uuid,
    created_at timestamp,
    PRIMARY KEY (following_id, follower_id)
) 
WITH CLUSTERING ORDER BY (follower_id DESC)
AND comment = 'Table miroir optimisee pour lister les abonnes d un utilisateur';