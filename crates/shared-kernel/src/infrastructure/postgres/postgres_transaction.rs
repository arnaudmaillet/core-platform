// crates/shared-kernel/src/persistence/postgres/postgres_tx

use sqlx::{Postgres, Transaction as PostgresTx};
use crate::domain::transaction::Transaction;

pub struct PostgresTransaction {
    inner: PostgresTx<'static, Postgres>,
}

impl PostgresTransaction {
    pub fn new(tx: PostgresTx<'static, Postgres>) -> Self {
        Self { inner: tx }
    }
    pub fn get_mut(&mut self) -> &mut PostgresTx<'static, Postgres> {
        &mut self.inner
    }
}

impl Transaction for PostgresTransaction {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

