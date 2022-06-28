use std::ffi::c_void;

use crate::{
    ffi::{copy_hash_bytes, copy_hash_or_account_bytes, FfiStream, NetworkConstantsDto},
    messages::BulkPull,
    BlockHash, HashOrAccount,
};

use super::{
    create_message_handle, create_message_handle2, downcast_message, downcast_message_mut,
    MessageHandle, MessageHeaderHandle,
};

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle(constants, BulkPull::new)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_create2(
    header: *mut MessageHeaderHandle,
) -> *mut MessageHandle {
    create_message_handle2(header, BulkPull::with_header)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_size() -> usize {
    BulkPull::serialized_size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_start(handle: *mut MessageHandle, start: *mut u8) {
    copy_hash_or_account_bytes(downcast_message::<BulkPull>(handle).start, start);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_set_start(
    handle: *mut MessageHandle,
    start: *const u8,
) {
    downcast_message_mut::<BulkPull>(handle).start = HashOrAccount::from(start);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_end(handle: *mut MessageHandle, end: *mut u8) {
    copy_hash_bytes(downcast_message::<BulkPull>(handle).end, end);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_set_end(handle: *mut MessageHandle, end: *const u8) {
    downcast_message_mut::<BulkPull>(handle).end = BlockHash::from(end);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_count(handle: *mut MessageHandle) -> u32 {
    downcast_message::<BulkPull>(handle).count
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_set_count(handle: *mut MessageHandle, count: u32) {
    downcast_message_mut::<BulkPull>(handle).count = count;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_serialize(
    handle: *mut MessageHandle,
    stream: *mut c_void,
) -> bool {
    let mut stream = FfiStream::new(stream);
    downcast_message::<BulkPull>(handle)
        .serialize(&mut stream)
        .is_ok()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_deserialize(
    handle: *mut MessageHandle,
    stream: *mut c_void,
) -> bool {
    let mut stream = FfiStream::new(stream);
    downcast_message_mut::<BulkPull>(handle)
        .deserialize(&mut stream)
        .is_ok()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_is_count_present(
    handle: *mut MessageHandle,
) -> bool {
    downcast_message::<BulkPull>(handle).is_count_present()
}