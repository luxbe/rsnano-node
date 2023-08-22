use std::{
    ffi::{c_char, c_void, CStr},
    net::{Ipv6Addr, SocketAddr, SocketAddrV6},
    sync::{atomic::Ordering, Arc},
};

use rsnano_core::{
    utils::{system_time_as_nanoseconds, system_time_from_nanoseconds},
    PublicKey,
};
use rsnano_node::transport::{ChannelEnum, TcpChannels, TcpEndpointAttempt};

use crate::{
    bootstrap::TcpServerHandle,
    utils::{ptr_into_ipv6addr, ContainerInfoComponentHandle, ContextWrapper},
    NetworkConstantsDto, VoidPointerCallback,
};

use super::{ChannelHandle, EndpointDto, SocketHandle};

pub struct TcpChannelsHandle(Arc<TcpChannels>);

#[no_mangle]
pub extern "C" fn rsn_tcp_channels_create(
    port: u16,
    network_constants: &NetworkConstantsDto,
    allow_local_peers: bool,
) -> *mut TcpChannelsHandle {
    Box::into_raw(Box::new(TcpChannelsHandle(Arc::new(TcpChannels::new(
        port,
        network_constants.try_into().unwrap(),
        allow_local_peers,
    )))))
}

#[no_mangle]
pub extern "C" fn rsn_tcp_channels_set_port(handle: &mut TcpChannelsHandle, port: u16) {
    handle.0.port.store(port, Ordering::SeqCst)
}

pub type NewChannelCallback = unsafe extern "C" fn(*mut c_void, *mut ChannelHandle);

#[no_mangle]
pub extern "C" fn rsn_tcp_channels_stop(handle: &mut TcpChannelsHandle) {
    handle.0.stop();
}

#[no_mangle]
pub extern "C" fn rsn_tcp_channels_on_new_channel(
    handle: &mut TcpChannelsHandle,
    callback_handle: *mut c_void,
    call_callback: NewChannelCallback,
    delete_callback: VoidPointerCallback,
) {
    let context_wrapper = ContextWrapper::new(callback_handle, delete_callback);
    let callback = Arc::new(move |channel| {
        let ctx = context_wrapper.get_context();
        unsafe { call_callback(ctx, ChannelHandle::new(channel)) };
    });
    handle.0.on_new_channel(callback)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_destroy(handle: *mut TcpChannelsHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_erase_attempt(
    handle: *mut TcpChannelsHandle,
    endpoint: &EndpointDto,
) {
    (*handle)
        .0
        .tcp_channels
        .lock()
        .unwrap()
        .attempts
        .remove(&endpoint.into());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_get_attempt_count_by_ip_address(
    handle: *mut TcpChannelsHandle,
    ipv6_bytes: *const u8,
) -> usize {
    (*handle)
        .0
        .tcp_channels
        .lock()
        .unwrap()
        .attempts
        .count_by_address(&ptr_into_ipv6addr(ipv6_bytes))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_get_attempt_count_by_subnetwork(
    handle: *mut TcpChannelsHandle,
    ipv6_bytes: *const u8,
) -> usize {
    (*handle)
        .0
        .tcp_channels
        .lock()
        .unwrap()
        .attempts
        .count_by_subnetwork(&ptr_into_ipv6addr(ipv6_bytes))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_add_attempt(
    handle: *mut TcpChannelsHandle,
    endpoint: &EndpointDto,
) -> bool {
    let attempt = TcpEndpointAttempt::new(endpoint.into());
    let mut guard = (*handle).0.tcp_channels.lock().unwrap();
    guard.attempts.insert(attempt.into())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_attempts_count(handle: *mut TcpChannelsHandle) -> usize {
    let guard = (*handle).0.tcp_channels.lock().unwrap();
    guard.attempts.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_purge(handle: *mut TcpChannelsHandle, cutoff_ns: u64) {
    let cutoff = system_time_from_nanoseconds(cutoff_ns);
    let mut guard = (*handle).0.tcp_channels.lock().unwrap();
    guard.purge(cutoff)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_erase_channel_by_node_id(
    handle: &mut TcpChannelsHandle,
    node_id: *const u8,
) {
    handle
        .0
        .tcp_channels
        .lock()
        .unwrap()
        .channels
        .remove_by_node_id(&PublicKey::from_ptr(node_id))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_erase_channel_by_endpoint(
    handle: &mut TcpChannelsHandle,
    endpoint: &EndpointDto,
) {
    handle
        .0
        .tcp_channels
        .lock()
        .unwrap()
        .channels
        .remove_by_endpoint(&SocketAddr::from(endpoint))
}

#[no_mangle]
pub extern "C" fn rsn_tcp_channels_channel_exists(
    handle: &mut TcpChannelsHandle,
    endpoint: &EndpointDto,
) -> bool {
    handle
        .0
        .tcp_channels
        .lock()
        .unwrap()
        .channels
        .exists(&SocketAddr::from(endpoint))
}

#[no_mangle]
pub extern "C" fn rsn_tcp_channels_channel_count(handle: &mut TcpChannelsHandle) -> usize {
    handle.0.tcp_channels.lock().unwrap().channels.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_insert(
    handle: &mut TcpChannelsHandle,
    channel: &ChannelHandle,
    socket: &SocketHandle,
    server: *const TcpServerHandle,
) -> bool {
    let server = if server.is_null() {
        None
    } else {
        Some((*server).0.clone())
    };
    handle.0.insert(&channel.0, &socket.0, server).is_err()
}

#[no_mangle]
pub extern "C" fn rsn_tcp_channels_bootstrap_peer(
    handle: &mut TcpChannelsHandle,
    result: &mut EndpointDto,
) {
    let peer = handle.0.tcp_channels.lock().unwrap().bootstrap_peer();
    *result = peer.into();
}

#[no_mangle]
pub extern "C" fn rsn_tcp_channels_close_channels(handle: &mut TcpChannelsHandle) {
    handle.0.tcp_channels.lock().unwrap().close_channels();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_count_by_ip(
    handle: &mut TcpChannelsHandle,
    ip: *const u8,
) -> usize {
    let address_bytes: [u8; 16] = std::slice::from_raw_parts(ip, 16).try_into().unwrap();
    let ip_address = Ipv6Addr::from(address_bytes);
    handle
        .0
        .tcp_channels
        .lock()
        .unwrap()
        .channels
        .count_by_ip(&ip_address)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_count_by_subnet(
    handle: &mut TcpChannelsHandle,
    subnet: *const u8,
) -> usize {
    let address_bytes: [u8; 16] = std::slice::from_raw_parts(subnet, 16).try_into().unwrap();
    let subnet = Ipv6Addr::from(address_bytes);
    handle
        .0
        .tcp_channels
        .lock()
        .unwrap()
        .channels
        .count_by_subnet(&subnet)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_list_channels(
    handle: &mut TcpChannelsHandle,
    min_version: u8,
    include_temporary_channels: bool,
) -> *mut ChannelListHandle {
    let channels = handle
        .0
        .tcp_channels
        .lock()
        .unwrap()
        .list(min_version, include_temporary_channels);
    Box::into_raw(Box::new(ChannelListHandle(channels)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_keepalive_list(
    handle: &mut TcpChannelsHandle,
) -> *mut ChannelListHandle {
    let channels = handle.0.tcp_channels.lock().unwrap().keepalive_list();
    Box::into_raw(Box::new(ChannelListHandle(channels)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_update_channel(
    handle: &mut TcpChannelsHandle,
    endpoint: &EndpointDto,
) {
    handle
        .0
        .tcp_channels
        .lock()
        .unwrap()
        .update(&endpoint.into())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_set_last_packet_sent(
    handle: &mut TcpChannelsHandle,
    endpoint: &EndpointDto,
    time_ns: u64,
) {
    handle
        .0
        .tcp_channels
        .lock()
        .unwrap()
        .set_last_packet_sent(&endpoint.into(), system_time_from_nanoseconds(time_ns));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_not_a_peer(
    handle: &mut TcpChannelsHandle,
    endpoint: &EndpointDto,
    allow_local_peers: bool,
) -> bool {
    handle.0.not_a_peer(&endpoint.into(), allow_local_peers)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_find_channel(
    handle: &mut TcpChannelsHandle,
    endpoint: &EndpointDto,
) -> *mut ChannelHandle {
    match handle.0.find_channel(&endpoint.into()) {
        Some(channel) => ChannelHandle::new(channel),
        None => std::ptr::null_mut(),
    }
}
#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_random_channels(
    handle: &mut TcpChannelsHandle,
    count: usize,
    min_version: u8,
    include_temporary_channels: bool,
) -> *mut ChannelListHandle {
    let channels = handle
        .0
        .random_channels(count, min_version, include_temporary_channels);
    Box::into_raw(Box::new(ChannelListHandle(channels)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_get_peers(
    handle: &mut TcpChannelsHandle,
) -> *mut EndpointListHandle {
    let peers = handle.0.get_peers();
    Box::into_raw(Box::new(EndpointListHandle(peers)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_get_first_channel(
    handle: &mut TcpChannelsHandle,
) -> *mut ChannelHandle {
    ChannelHandle::new(handle.0.get_first_channel().unwrap())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_find_node_id(
    handle: &mut TcpChannelsHandle,
    node_id: *const u8,
) -> *mut ChannelHandle {
    let node_id = PublicKey::from_ptr(node_id);
    match handle.0.find_node_id(&node_id) {
        Some(channel) => ChannelHandle::new(channel),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_collect_container_info(
    handle: &TcpChannelsHandle,
    name: *const c_char,
) -> *mut ContainerInfoComponentHandle {
    let container_info = (*handle)
        .0
        .collect_container_info(CStr::from_ptr(name).to_str().unwrap().to_owned());
    Box::into_raw(Box::new(ContainerInfoComponentHandle(container_info)))
}

pub struct EndpointListHandle(Vec<SocketAddr>);

#[no_mangle]
pub unsafe extern "C" fn rsn_endpoint_list_len(handle: &EndpointListHandle) -> usize {
    handle.0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_endpoint_list_get(
    handle: &EndpointListHandle,
    index: usize,
    result: &mut EndpointDto,
) {
    *result = handle.0.get(index).unwrap().into();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_endpoint_list_destroy(handle: *mut EndpointListHandle) {
    drop(Box::from_raw(handle))
}

pub struct TcpEndpointAttemptDto {
    pub endpoint: EndpointDto,
    pub address: [u8; 16],
    pub subnetwork: [u8; 16],
    pub last_attempt: u64,
}

impl From<&TcpEndpointAttemptDto> for TcpEndpointAttempt {
    fn from(value: &TcpEndpointAttemptDto) -> Self {
        Self {
            endpoint: SocketAddrV6::from(&value.endpoint),
            address: Ipv6Addr::from(value.address),
            subnetwork: Ipv6Addr::from(value.subnetwork),
            last_attempt: system_time_from_nanoseconds(value.last_attempt),
        }
    }
}

impl From<&TcpEndpointAttempt> for TcpEndpointAttemptDto {
    fn from(value: &TcpEndpointAttempt) -> Self {
        Self {
            endpoint: value.endpoint.into(),
            address: value.address.octets(),
            subnetwork: value.subnetwork.octets(),
            last_attempt: system_time_as_nanoseconds(value.last_attempt),
        }
    }
}

pub struct ChannelListHandle(Vec<Arc<ChannelEnum>>);

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_list_len(handle: *mut ChannelListHandle) -> usize {
    (*handle).0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_list_get(
    handle: *mut ChannelListHandle,
    index: usize,
) -> *mut ChannelHandle {
    ChannelHandle::new((*handle).0[index].clone())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_list_destroy(handle: *mut ChannelListHandle) {
    drop(Box::from_raw(handle))
}
