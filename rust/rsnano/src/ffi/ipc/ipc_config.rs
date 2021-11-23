use crate::{
    config::NetworkConstants,
    ffi::config::{fill_network_constants_dto, NetworkConstantsDto},
    ipc::{IpcConfig, IpcConfigTransport},
};
use std::{convert::TryFrom, os::unix::prelude::OsStrExt};

#[repr(C)]
pub struct IpcConfigTransportDto {
    pub enabled: bool,
    pub allow_unsafe: bool,
    pub io_timeout: usize,
    pub io_threads: i64,
}

#[repr(C)]
pub struct IpcConfigDto {
    pub domain_transport: IpcConfigTransportDto,
    pub domain_path: [u8; 512],
    pub domain_path_len: usize,
    pub tcp_transport: IpcConfigTransportDto,
    pub tcp_network_constants: NetworkConstantsDto,
    pub tcp_port: u16,
    pub flatbuffers_skip_unexpected_fields_in_json: bool,
    pub flatbuffers_verify_buffers: bool,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ipc_config_create(
    dto: *mut IpcConfigDto,
    network_constants: &NetworkConstantsDto,
) -> i32 {
    let network_constants = match NetworkConstants::try_from(network_constants) {
        Ok(n) => n,
        Err(_) => return -1,
    };
    let config = IpcConfig::new(&network_constants);
    let dto = &mut (*dto);
    fill_config_transport_dto(
        &mut dto.domain_transport,
        &config.transport_domain.transport,
    );
    let bytes = config.transport_domain.path.as_os_str().as_bytes();
    if bytes.len() > dto.domain_path.len() {
        return -1;
    }
    dto.domain_path[..bytes.len()].copy_from_slice(bytes);
    dto.domain_path_len = bytes.len();
    fill_config_transport_dto(&mut dto.tcp_transport, &config.transport_tcp.transport);
    fill_network_constants_dto(
        &mut dto.tcp_network_constants,
        &config.transport_tcp.network_constants,
    );
    dto.tcp_port = config.transport_tcp.port;
    dto.flatbuffers_skip_unexpected_fields_in_json =
        config.flatbuffers.skip_unexpected_fields_in_json;
    dto.flatbuffers_verify_buffers = config.flatbuffers.verify_buffers;

    0
}

fn fill_config_transport_dto(dto: &mut IpcConfigTransportDto, cfg: &IpcConfigTransport) {
    dto.enabled = cfg.enabled;
    dto.allow_unsafe = cfg.allow_unsafe;
    dto.io_timeout = cfg.io_timeout;
    dto.io_threads = cfg.io_threads;
}