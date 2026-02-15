// crates/profile/src/infrastructure/utils/infrastructure_profile_test_context.rs

use shared_kernel::infrastructure::utils::InfrastructureKernelTestContext;
use crate::infrastructure::utils::infrastructure_profile_test_context_builder::InfrastructureProfileTestBuilder;

pub struct InfrastructureProfileTestContext {
    kernel: InfrastructureKernelTestContext,
}

impl InfrastructureProfileTestContext {
    pub fn builder() -> InfrastructureProfileTestBuilder {
        InfrastructureProfileTestBuilder::new()
    }

    /// Raccourci pour le cas le plus courant (setup standard du profil)
    pub async fn setup() -> Self {
        Self::builder().build().await
    }

    /// Getter pour accéder aux ressources du noyau (Postgres, Scylla, Redis)
    pub fn kernel(&self) -> &InfrastructureKernelTestContext {
        &self.kernel
    }

    /// Constructeur interne utilisé par le builder
    pub(crate) fn new(kernel: InfrastructureKernelTestContext) -> Self {
        Self { kernel }
    }
}