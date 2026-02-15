// crates/profile/src/infrastructure/utils/infrastructure_profile_test_context_builder.rs

use shared_kernel::infrastructure::utils::InfrastructureKernelTestBuilder;
use crate::infrastructure::utils::InfrastructureProfileTestContext;

pub struct InfrastructureProfileTestBuilder {
    kernel_builder: InfrastructureKernelTestBuilder,
}

impl InfrastructureProfileTestBuilder {
    pub fn new() -> Self {
        // On configure les migrations par défaut du module Profile ici
        let kernel_builder = InfrastructureKernelTestBuilder::new()
            .with_postgres_migrations(&["./migrations/postgres"])
            .with_scylla_migrations(&["./migrations/scylla"]);

        Self { kernel_builder }
    }

    /// Permet de surcharger ou d'ajouter des migrations si nécessaire
    pub fn with_extra_postgres(mut self, paths: &[&str]) -> Self {
        self.kernel_builder = self.kernel_builder.with_postgres_migrations(paths);
        self
    }

    pub fn with_extra_scylla(mut self, paths: &[&str]) -> Self {
        self.kernel_builder = self.kernel_builder.with_scylla_migrations(paths);
        self
    }

    /// Construit le contexte en encapsulant le noyau technique
    pub async fn build(self) -> InfrastructureProfileTestContext {
        let kernel = self.kernel_builder.build().await;
        InfrastructureProfileTestContext::new(kernel)
    }
}