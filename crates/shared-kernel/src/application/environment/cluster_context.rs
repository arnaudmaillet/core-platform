// crates/shared-kernel/src/environment.rs

use crate::types::Region;
use std::fmt;

/// `ClusterContext` représente l'identité et le périmètre d'exécution physique
/// du nœud d'infrastructure actif (le binaire en cours d'exécution).
///
/// C'est un *Value Object* essentiel pour l'expérience développeur (DX) et l'évolutivité.
/// Il encapsule les métadonnées de localisation nécessaires au Sharding et à la résilience
/// sans coupler le code métier aux variables d'environnement brutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClusterContext {
    region: Region,
    // 💡 Prêt pour le futur : Tu pourras ajouter ici 'environment: Env' (prod, staging)
    // ou 'shard_id: u16' sans jamais casser les signatures de tes cas d'usage ou builders.
}

impl ClusterContext {
    /// Crée un nouveau contexte de cluster à partir d'une région géographique d'ancrage.
    pub fn new(region: Region) -> Self {
        Self { region }
    }

    /// Extrait le contexte depuis les variables d'environnement du système (ex: `CLUSTER_REGION=EU`).
    /// Idéal pour l'initialisation propre dans le `main.rs` de tes services.
    pub fn from_env() -> Result<Self, String> {
        let region_str = std::env::var("CLUSTER_REGION").map_err(|_| {
            "La variable d'environnement 'CLUSTER_REGION' est manquante".to_string()
        })?;

        let region = Region::try_from(region_str.as_str())
            .map_err(|e| format!("Région de cluster invalide: {}", e))?;

        Ok(Self { region })
    }

    /// Retourne la région géographique associée à ce contexte de cluster.
    pub fn region(&self) -> Region {
        self.region
    }
}

impl fmt::Display for ClusterContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ClusterContext(region: {})", self.region)
    }
}
