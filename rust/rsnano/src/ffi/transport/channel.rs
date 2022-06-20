use std::sync::Arc;

use crate::{
    transport::{Channel, ChannelInProc, ChannelTcp, ChannelUdp},
    Account,
};

pub enum ChannelType {
    Tcp(ChannelTcp),
    InProc(ChannelInProc),
    Udp(ChannelUdp),
}

pub struct ChannelHandle(Arc<ChannelType>);

impl ChannelHandle {
    pub fn new(channel: Arc<ChannelType>) -> Self {
        Self(channel)
    }
}

pub unsafe fn as_tcp_channel(handle: *mut ChannelHandle) -> &'static ChannelTcp {
    match (*handle).0.as_ref() {
        ChannelType::Tcp(tcp) => tcp,
        _ => panic!("expected tcp channel"),
    }
}

pub unsafe fn as_channel(handle: *mut ChannelHandle) -> &'static dyn Channel {
    match (*handle).0.as_ref() {
        ChannelType::Tcp(tcp) => tcp,
        ChannelType::InProc(inproc) => inproc,
        ChannelType::Udp(udp) => udp,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_destroy(handle: *mut ChannelHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_is_temporary(handle: *mut ChannelHandle) -> bool {
    as_channel(handle).is_temporary()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_set_temporary(handle: *mut ChannelHandle, temporary: bool) {
    as_channel(handle).set_temporary(temporary);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_get_last_bootstrap_attempt(handle: *mut ChannelHandle) -> u64 {
    as_channel(handle).get_last_bootstrap_attempt()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_set_last_bootstrap_attempt(
    handle: *mut ChannelHandle,
    instant: u64,
) {
    as_channel(handle).set_last_bootstrap_attempt(instant);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_get_last_packet_received(handle: *mut ChannelHandle) -> u64 {
    as_channel(handle).get_last_packet_received()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_set_last_packet_received(
    handle: *mut ChannelHandle,
    instant: u64,
) {
    as_channel(handle).set_last_packet_received(instant);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_get_last_packet_sent(handle: *mut ChannelHandle) -> u64 {
    as_channel(handle).get_last_packet_sent()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_set_last_packet_sent(
    handle: *mut ChannelHandle,
    instant: u64,
) {
    as_channel(handle).set_last_packet_sent(instant);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_get_node_id(
    handle: *mut ChannelHandle,
    result: *mut u8,
) -> bool {
    match as_channel(handle).get_node_id() {
        Some(id) => {
            std::slice::from_raw_parts_mut(result, 32).copy_from_slice(id.as_bytes());
            true
        }
        None => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_set_node_id(handle: *mut ChannelHandle, id: *const u8) {
    as_channel(handle).set_node_id(Account::from(id));
}

#[no_mangle]
pub extern "C" fn rsn_channel_inproc_create(now: u64) -> *mut ChannelHandle {
    Box::into_raw(Box::new(ChannelHandle(Arc::new(ChannelType::InProc(
        ChannelInProc::new(now),
    )))))
}

#[no_mangle]
pub extern "C" fn rsn_channel_udp_create(now: u64) -> *mut ChannelHandle {
    Box::into_raw(Box::new(ChannelHandle(Arc::new(ChannelType::Udp(
        ChannelUdp::new(now),
    )))))
}