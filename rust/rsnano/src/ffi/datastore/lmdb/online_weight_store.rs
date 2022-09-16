use std::sync::Arc;

use crate::{
    datastore::{lmdb::LmdbOnlineWeightStore, OnlineWeightStore},
    Amount,
};

use super::{
    iterator::{to_lmdb_iterator_handle, LmdbIteratorHandle},
    lmdb_env::LmdbEnvHandle,
    TransactionHandle,
};

pub struct LmdbOnlineWeightStoreHandle(LmdbOnlineWeightStore);

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_online_weight_store_create(
    env_handle: *mut LmdbEnvHandle,
) -> *mut LmdbOnlineWeightStoreHandle {
    Box::into_raw(Box::new(LmdbOnlineWeightStoreHandle(
        LmdbOnlineWeightStore::new(Arc::clone(&*env_handle)),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_online_weight_store_destroy(
    handle: *mut LmdbOnlineWeightStoreHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_online_weight_store_table_handle(
    handle: *mut LmdbOnlineWeightStoreHandle,
) -> u32 {
    (*handle).0.table_handle
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_online_weight_store_set_table_handle(
    handle: *mut LmdbOnlineWeightStoreHandle,
    table_handle: u32,
) {
    (*handle).0.table_handle = table_handle;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_online_weight_store_put(
    handle: *mut LmdbOnlineWeightStoreHandle,
    txn: *mut TransactionHandle,
    time: u64,
    amount: *const u8,
) {
    (*handle)
        .0
        .put((*txn).as_write_txn(), time, &Amount::from_ptr(amount));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_online_weight_store_del(
    handle: *mut LmdbOnlineWeightStoreHandle,
    txn: *mut TransactionHandle,
    time: u64,
) {
    (*handle).0.del((*txn).as_write_txn(), time);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_online_weight_store_begin(
    handle: *mut LmdbOnlineWeightStoreHandle,
    txn: *mut TransactionHandle,
) -> *mut LmdbIteratorHandle {
    let mut iterator = (*handle).0.begin((*txn).as_txn());
    to_lmdb_iterator_handle(iterator.as_mut())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_online_weight_store_rbegin(
    handle: *mut LmdbOnlineWeightStoreHandle,
    txn: *mut TransactionHandle,
) -> *mut LmdbIteratorHandle {
    let mut iterator = (*handle).0.rbegin((*txn).as_txn());
    to_lmdb_iterator_handle(iterator.as_mut())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_online_weight_store_count(
    handle: *mut LmdbOnlineWeightStoreHandle,
    txn: *mut TransactionHandle,
) -> usize {
    (*handle).0.count((*txn).as_txn())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_online_weight_store_clear(
    handle: *mut LmdbOnlineWeightStoreHandle,
    txn: *mut TransactionHandle,
) {
    (*handle).0.clear((*txn).as_write_txn())
}