-- ==========================================
-- 1. INFRASTRUCTURE GLOBALE (Toujours explicite)
-- ==========================================
CREATE KEYSPACE IF NOT EXISTS global_routing
WITH replication = {
    'class': 'SimpleStrategy',
    'replication_factor': 1
};

-- Table 1 : Routage principal
CREATE TABLE IF NOT EXISTS global_routing.profiles (
    profile_id uuid,
    region text,
    PRIMARY KEY (profile_id)
);

-- Table 2 : Routage et Unicité Mondiale du Handle
CREATE TABLE IF NOT EXISTS global_routing.slugs (
    slug_hash text,
    profile_id uuid,
    region text,
    PRIMARY KEY (slug_hash)
);

-- ==========================================
-- 2. INFRASTRUCTURE RÉGIONALE (Agnostique)
-- ==========================================
-- 💡 Le Keyspace sera créé et configuré dynamiquement par l'Orchestrateur en Rust.
-- Les tables ci-dessous s'appliqueront dans le keyspace courant (via la commande USE).

-- Table 3 : La table de vérité du Profil
CREATE TABLE IF NOT EXISTS profiles (
    id uuid,
    account_id uuid,
    handle text,
    display_name text,
    bio text,
    avatar_url text,
    banner_url text,
    location_label text,
    social_links map<text, text>,
    is_private boolean,
    version bigint,
    created_at timestamp,
    updated_at timestamp,
    PRIMARY KEY (id)
) WITH cdc = {'enabled': true};

-- Table 4 : L'index de performance
CREATE TABLE IF NOT EXISTS profiles_by_account (
    account_id uuid,
    profile_id uuid,
    handle text,
    display_name text,
    avatar_url text,
    is_private boolean,
    PRIMARY KEY (account_id, profile_id)
);