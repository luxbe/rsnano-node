use num::FromPrimitive;

use super::{
    bandwidth_limiter::OutboundBandwidthLimiterHandle,
    channel::{as_tcp_channel, ChannelHandle},
    channel_tcp_observer::FfiChannelTcpObserverWeakPtr,
    socket::SocketHandle,
    EndpointDto,
};
use crate::{messages::MessageHandle, utils::FfiIoContext, ErrorCodeDto, VoidPointerCallback};
use rsnano_node::{
    transport::{BufferDropPolicy, Channel, ChannelEnum, ChannelTcp, TrafficType},
    utils::ErrorCode,
};
use std::{ffi::c_void, net::SocketAddr, ops::Deref, sync::Arc, time::SystemTime};

#[no_mangle]
/// observer is `weak_ptr<channel_tcp_observer> *`
/// io_ctx is `boost::asio::io_context *`
pub unsafe extern "C" fn rsn_channel_tcp_create(
    socket: *mut SocketHandle,
    observer: *mut c_void,
    limiter: *const OutboundBandwidthLimiterHandle,
    io_ctx: *mut c_void,
    channel_id: usize,
) -> *mut ChannelHandle {
    let observer = Arc::new(FfiChannelTcpObserverWeakPtr::new(observer));
    let limiter = Arc::clone(&*limiter);
    let io_ctx = Arc::new(FfiIoContext::new(io_ctx));
    ChannelHandle::new(Arc::new(ChannelEnum::Tcp(ChannelTcp::new(
        (*socket).deref(),
        SystemTime::now(),
        observer,
        limiter,
        io_ctx,
        channel_id,
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_local_endpoint(
    handle: *mut ChannelHandle,
    endpoint: *mut EndpointDto,
) {
    (*endpoint) = EndpointDto::from(as_tcp_channel(handle).local_endpoint());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_remote_endpoint(
    handle: *mut ChannelHandle,
    endpoint: *mut EndpointDto,
) {
    (*endpoint) = EndpointDto::from(as_tcp_channel(handle).remote_endpoint())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_peering_endpoint(
    handle: *mut ChannelHandle,
    endpoint: *mut EndpointDto,
) {
    (*endpoint) = EndpointDto::from(as_tcp_channel(handle).peering_endpoint())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_set_peering_endpoint(
    handle: *mut ChannelHandle,
    endpoint: *const EndpointDto,
) {
    let address = SocketAddr::from(&*endpoint);
    as_tcp_channel(handle).set_peering_endpoint(address);
}

pub type ChannelTcpSendBufferCallback =
    unsafe extern "C" fn(*mut c_void, *const ErrorCodeDto, usize);

pub struct SendBufferCallbackWrapper {
    callback: ChannelTcpSendBufferCallback,
    /// `std::function<error_code const&, size_t>*`
    context: *mut c_void,
    delete_callback: VoidPointerCallback,
}

impl SendBufferCallbackWrapper {
    pub fn new(
        callback: ChannelTcpSendBufferCallback,
        context: *mut c_void,
        delete_callback: VoidPointerCallback,
    ) -> Self {
        Self {
            callback,
            context,
            delete_callback,
        }
    }

    pub fn call(&self, ec: ErrorCode, size: usize) {
        let ec_dto = ErrorCodeDto::from(&ec);
        unsafe {
            (self.callback)(self.context, &ec_dto, size);
        }
    }
}

impl Drop for SendBufferCallbackWrapper {
    fn drop(&mut self) {
        unsafe {
            (self.delete_callback)(self.context);
        }
    }
}

unsafe impl Send for SendBufferCallbackWrapper {}
unsafe impl Sync for SendBufferCallbackWrapper {}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_send_buffer(
    handle: *mut ChannelHandle,
    buffer: *const u8,
    buffer_len: usize,
    callback: ChannelTcpSendBufferCallback,
    delete_callback: VoidPointerCallback,
    callback_context: *mut c_void,
    policy: u8,
    traffic_type: u8,
) {
    let buffer = Arc::new(std::slice::from_raw_parts(buffer, buffer_len).to_vec());
    let callback_wrapper =
        SendBufferCallbackWrapper::new(callback, callback_context, delete_callback);
    let cb = Box::new(move |ec, size| {
        callback_wrapper.call(ec, size);
    });
    let policy = BufferDropPolicy::from_u8(policy).unwrap();
    let traffic_type = TrafficType::from_u8(traffic_type).unwrap();
    as_tcp_channel(handle).send_buffer(&buffer, Some(cb), policy, traffic_type);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_network_version(handle: *mut ChannelHandle) -> u8 {
    let tcp = as_tcp_channel(handle);
    tcp.network_version()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_network_set_version(
    handle: *mut ChannelHandle,
    version: u8,
) {
    let tcp = as_tcp_channel(handle);
    tcp.set_network_version(version)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_eq(a: *mut ChannelHandle, b: *mut ChannelHandle) -> bool {
    as_tcp_channel(a).eq(as_tcp_channel(b))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_max(handle: *mut ChannelHandle, traffic_type: u8) -> bool {
    as_tcp_channel(handle).max(FromPrimitive::from_u8(traffic_type).unwrap())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_is_alive(handle: *mut ChannelHandle) -> bool {
    as_tcp_channel(handle).is_alive()
}

pub type ChannelTcpSendCallback = unsafe extern "C" fn(*mut c_void, *const ErrorCodeDto, usize);

pub struct ChannelTcpSendCallbackWrapper {
    context: *mut c_void,
    callback: ChannelTcpSendCallback,
    delete: VoidPointerCallback,
}

impl ChannelTcpSendCallbackWrapper {
    pub fn new(
        context: *mut c_void,
        callback: ChannelTcpSendCallback,
        delete: VoidPointerCallback,
    ) -> Self {
        Self {
            context,
            callback,
            delete,
        }
    }

    pub fn call(&self, ec: ErrorCode, size: usize) {
        let ec_dto = ErrorCodeDto::from(&ec);
        unsafe {
            (self.callback)(self.context, &ec_dto, size);
        }
    }
}

impl Drop for ChannelTcpSendCallbackWrapper {
    fn drop(&mut self) {
        unsafe {
            (self.delete)(self.context);
        }
    }
}

unsafe impl Send for ChannelTcpSendCallbackWrapper {}
unsafe impl Sync for ChannelTcpSendCallbackWrapper {}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_send(
    handle: *mut ChannelHandle,
    msg: *mut MessageHandle,
    callback: ChannelTcpSendCallback,
    delete_callback: VoidPointerCallback,
    context: *mut c_void,
    policy: u8,
    traffic_type: u8,
) {
    let callback_wrapper = ChannelTcpSendCallbackWrapper::new(context, callback, delete_callback);
    let callback_box = Box::new(move |ec, size| {
        callback_wrapper.call(ec, size);
    });
    as_tcp_channel(handle).send(
        (*msg).as_ref(),
        Some(callback_box),
        BufferDropPolicy::from_u8(policy).unwrap(),
        TrafficType::from_u8(traffic_type).unwrap(),
    );
}
