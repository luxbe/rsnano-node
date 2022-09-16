use std::sync::Arc;

use rand::{thread_rng, Rng};

use crate::{
    datastore::{
        parallel_traversal, DbIterator, NullIterator, PrunedStore, ReadTransaction, Transaction,
        WriteTransaction,
    },
    BlockHash, NoValue,
};

use super::{
    assert_success, exists, get_raw_lmdb_txn, mdb_count, mdb_del, mdb_drop, mdb_put, LmdbEnv,
    LmdbIterator, MdbVal,
};

pub struct LmdbPrunedStore {
    env: Arc<LmdbEnv>,
    pub table_handle: u32,
}

impl LmdbPrunedStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            env,
            table_handle: 0,
        }
    }
}

impl PrunedStore for LmdbPrunedStore {
    fn put(&self, txn: &dyn WriteTransaction, hash: &BlockHash) {
        let status = unsafe {
            mdb_put(
                get_raw_lmdb_txn(txn.as_transaction()),
                self.table_handle,
                &mut MdbVal::from(hash),
                &mut MdbVal::new(),
                0,
            )
        };
        assert_success(status);
    }

    fn del(&self, txn: &dyn WriteTransaction, hash: &BlockHash) {
        let status = unsafe {
            mdb_del(
                get_raw_lmdb_txn(txn.as_transaction()),
                self.table_handle,
                &mut MdbVal::from(hash),
                None,
            )
        };
        assert_success(status);
    }

    fn exists(&self, txn: &dyn Transaction, hash: &BlockHash) -> bool {
        exists(txn, self.table_handle, &mut hash.into())
    }

    fn begin(&self, txn: &dyn Transaction) -> Box<dyn DbIterator<BlockHash, NoValue>> {
        Box::new(LmdbIterator::new(txn, self.table_handle, None, true))
    }

    fn begin_at_hash(
        &self,
        txn: &dyn Transaction,
        hash: &BlockHash,
    ) -> Box<dyn DbIterator<BlockHash, NoValue>> {
        Box::new(LmdbIterator::new(txn, self.table_handle, Some(hash), true))
    }

    fn random(&self, txn: &dyn Transaction) -> BlockHash {
        let random_hash = BlockHash::from_bytes(thread_rng().gen());
        let mut existing = self.begin_at_hash(txn, &random_hash);
        if existing.is_end() {
            existing = self.begin(txn);
        }

        let result = existing.current().map(|(k, _)| *k).unwrap_or_default();
        result
    }

    fn count(&self, txn: &dyn Transaction) -> usize {
        unsafe { mdb_count(get_raw_lmdb_txn(txn), self.table_handle) }
    }

    fn clear(&self, txn: &dyn WriteTransaction) {
        let status =
            unsafe { mdb_drop(get_raw_lmdb_txn(txn.as_transaction()), self.table_handle, 0) };
        assert_success(status);
    }

    fn end(&self) -> Box<dyn DbIterator<BlockHash, NoValue>> {
        Box::new(NullIterator::new())
    }

    fn for_each_par(
        &self,
        action: &(dyn Fn(
            &dyn ReadTransaction,
            &mut dyn DbIterator<BlockHash, NoValue>,
            &mut dyn DbIterator<BlockHash, NoValue>,
        ) + Send
              + Sync),
    ) {
        parallel_traversal(&|start, end, is_last| {
            let mut transaction = self.env.tx_begin_read();
            let mut begin_it = self.begin_at_hash(&transaction, &start.into());
            let mut end_it = if !is_last {
                self.begin_at_hash(&transaction, &end.into())
            } else {
                self.end()
            };
            action(&mut transaction, begin_it.as_mut(), end_it.as_mut());
        });
    }
}