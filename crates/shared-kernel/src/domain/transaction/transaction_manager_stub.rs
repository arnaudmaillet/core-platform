// crates/shared-kernel/src/domain/transaction/transaction_manager_stub.rs

use std::pin::Pin;
use crate::domain::transaction::{Transaction, TransactionManager};
use crate::domain::transaction::transaction_stub::FakeTransaction;

pub struct StubTxManager;

impl TransactionManager for StubTxManager {
    fn in_transaction<'a>(
        &'a self,
        f: Box<
            dyn FnOnce(
                Box<dyn Transaction>,
            ) -> Pin<Box<dyn std::future::Future<Output =crate::errors::Result<()>> + Send + 'a>>
            + Send
            + 'a,
        >,
    ) -> Pin<Box<dyn Future<Output =crate::errors::Result<()>> + Send + 'a>> {
        // On crée l'instance ici pour qu'elle soit trouvée dans le scope
        let tx = Box::new(FakeTransaction);
        Box::pin(async move { f(tx).await })
    }
}