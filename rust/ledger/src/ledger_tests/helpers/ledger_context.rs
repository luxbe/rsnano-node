use std::sync::Arc;

use crate::{ledger_constants::LEDGER_CONSTANTS_STUB, Ledger};
use rsnano_core::{Account, ConfirmationHeightInfo};
use rsnano_store_lmdb::{EnvironmentWrapper, LmdbStore, LmdbWriteTransaction, TestDbFile};

use super::AccountBlockFactory;

pub(crate) struct LedgerContext {
    pub(crate) ledger: Ledger,
    _db_file: TestDbFile,
}

impl LedgerContext {
    pub fn empty() -> Self {
        let db_file = TestDbFile::random();
        let store = Arc::new(
            LmdbStore::<EnvironmentWrapper>::open(&db_file.path)
                .build()
                .unwrap(),
        );
        let ledger = Ledger::new(store.clone(), LEDGER_CONSTANTS_STUB.clone()).unwrap();

        LedgerContext {
            ledger,
            _db_file: db_file,
        }
    }

    pub fn genesis_block_factory(&self) -> AccountBlockFactory {
        AccountBlockFactory::genesis(&self.ledger)
    }

    pub fn block_factory(&self) -> AccountBlockFactory {
        AccountBlockFactory::new(&self.ledger)
    }

    pub fn inc_confirmation_height(&self, txn: &mut LmdbWriteTransaction, account: &Account) {
        let mut height = self
            .ledger
            .store
            .confirmation_height
            .get(txn, account)
            .unwrap_or_else(|| ConfirmationHeightInfo {
                height: 0,
                frontier: self.ledger.account_info(txn, account).unwrap().head,
            });
        height.height = height.height + 1;
        self.ledger
            .store
            .confirmation_height
            .put(txn, account, &height);
    }
}
