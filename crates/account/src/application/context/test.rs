// // crates/account/src/application/context_test_ext.rs (ou dans tes fichiers de tests)

// use std::sync::Arc;

// use shared_kernel::{
//     application::BaseAppContext,
//     domain::{
//         repositories::{CacheRepositoryStub, OutboxRepositoryStub},
//         types::{AccountId, RegionCode},
//     },
// };

// use crate::{
//     application::context::{AccountAppContext, AccountContext, AccountContextBuilder},
//     domain::repositories::AccountRepositoryStub,
// };

// pub trait AccountContextTestExt {
//     fn build_test(self) -> AccountContext;
// }

// impl AccountContextTestExt for AccountContextBuilder {
//     fn build_test(self) -> AccountContext {
//         let mut builder = self;

//         // 1. Handle Simple Value Objects
//         if builder.account_id().is_none() {
//             builder = builder.with_account_id(AccountId::new());
//         }

//         if builder.region().is_none() {
//             builder = builder.with_region(RegionCode::from_raw("EU".to_string()));
//         }

//         // 2. Handle the App Context and Repositories
//         if builder.app().is_none() {
//             // Fix: BaseAppContext::new now needs (pool, cache)
//             let base = BaseAppContext::new(None, Arc::new(CacheRepositoryStub::new()));

//             let app = AccountAppContext::new(
//                 base,
//                 Arc::new(AccountRepositoryStub::new()),
//                 Arc::new(OutboxRepositoryStub::new()),
//             );
//             builder = builder.with_app(app);
//         }

//         builder.build()
//     }
// }
