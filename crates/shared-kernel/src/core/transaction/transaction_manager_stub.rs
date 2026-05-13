// crates/shared-kernel/src/domain/transaction/transaction_manager_stub.rs

use std::pin::Pin;

use crate::core::{FakeTransaction, Transaction, transaction::TransactionManager};

pub struct StubTxManager;

impl TransactionManager for StubTxManager {
    fn in_transaction<'a>(
        &'a self,
        f: Box<
            dyn FnOnce(
                    Box<dyn Transaction>,
                ) -> Pin<
                    Box<dyn std::future::Future<Output = crate::core::Result<()>> + Send + 'a>,
                > + Send
                + 'a,
        >,
    ) -> Pin<Box<dyn Future<Output = crate::core::Result<()>> + Send + 'a>> {
        // On crée l'instance ici pour qu'elle soit trouvée dans le scope
        let tx = Box::new(FakeTransaction::new());
        Box::pin(async move { f(tx).await })
    }
}
