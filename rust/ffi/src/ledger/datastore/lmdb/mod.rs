mod account_store;
mod block_store;
mod confirmation_height_store;
mod final_vote_store;
mod frontier_store;
mod iterator;
mod lmdb_env;
mod online_weight_store;
mod peer_store;
mod pending_store;
mod pruned_store;
mod store;
mod version_store;
mod wallet_store;
mod wallets;

pub use pending_store::{PendingInfoDto, PendingKeyDto};
use rsnano_store_lmdb::{LmdbReadTransaction, LmdbWriteTransaction, RoCursorWrapper, Transaction};
use std::{ffi::c_void, ops::Deref};
pub use store::LmdbStoreHandle;

use crate::VoidPointerCallback;

pub struct TransactionHandle(TransactionType);

impl TransactionHandle {
    pub fn new(txn_type: TransactionType) -> *mut TransactionHandle {
        Box::into_raw(Box::new(TransactionHandle(txn_type)))
    }

    pub fn as_read_txn_mut(&mut self) -> &mut LmdbReadTransaction {
        match &mut self.0 {
            TransactionType::Read(tx) => tx,
            _ => panic!("invalid tx type"),
        }
    }

    #[allow(unused)]
    pub fn as_read_txn(&mut self) -> &LmdbReadTransaction {
        match &mut self.0 {
            TransactionType::Read(tx) => tx,
            TransactionType::ReadRef(tx) => tx,
            _ => panic!("invalid tx type"),
        }
    }

    pub fn as_write_txn(&mut self) -> &mut LmdbWriteTransaction {
        match &mut self.0 {
            TransactionType::Write(tx) => tx,
            _ => panic!("invalid tx type"),
        }
    }

    pub fn as_txn(
        &self,
    ) -> &dyn Transaction<Database = lmdb::Database, RoCursor = RoCursorWrapper> {
        match &self.0 {
            TransactionType::Read(t) => t,
            TransactionType::Write(t) => t,
            TransactionType::ReadRef(t) => *t,
        }
    }
}

impl Deref for TransactionHandle {
    type Target = TransactionType;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub enum TransactionType {
    Read(LmdbReadTransaction),
    ReadRef(&'static LmdbReadTransaction),
    Write(LmdbWriteTransaction),
}

static mut TXN_CALLBACKS_DESTROY: Option<VoidPointerCallback> = None;
pub type TxnStartCallback = unsafe extern "C" fn(*mut c_void, u64, bool);
pub type TxnEndCallback = unsafe extern "C" fn(*mut c_void, u64);
static mut TXN_START: Option<TxnStartCallback> = None;
static mut TXN_END: Option<TxnEndCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_txn_callbacks_destroy(f: VoidPointerCallback) {
    TXN_CALLBACKS_DESTROY = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_txn_callbacks_start(f: TxnStartCallback) {
    TXN_START = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_txn_callbacks_end(f: TxnEndCallback) {
    TXN_END = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_read_txn_destroy(handle: *mut TransactionHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_read_txn_reset(handle: *mut TransactionHandle) {
    (*handle).as_read_txn_mut().reset();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_read_txn_renew(handle: *mut TransactionHandle) {
    (*handle).as_read_txn_mut().renew();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_read_txn_refresh(handle: *mut TransactionHandle) {
    (*handle).as_read_txn_mut().refresh();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_write_txn_destroy(handle: *mut TransactionHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_write_txn_commit(handle: *mut TransactionHandle) {
    (*handle).as_write_txn().commit();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_write_txn_renew(handle: *mut TransactionHandle) {
    (*handle).as_write_txn().renew();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_write_txn_refresh(handle: *mut TransactionHandle) {
    (*handle).as_write_txn().refresh();
}

pub(crate) unsafe fn into_read_txn_handle(
    txn: &dyn Transaction<Database = lmdb::Database, RoCursor = RoCursorWrapper>,
) -> *mut TransactionHandle {
    TransactionHandle::new(TransactionType::ReadRef(std::mem::transmute::<
        &LmdbReadTransaction,
        &'static LmdbReadTransaction,
    >(
        txn.as_any().downcast_ref::<LmdbReadTransaction>().unwrap(),
    )))
}
