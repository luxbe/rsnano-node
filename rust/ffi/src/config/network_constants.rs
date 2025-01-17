use num::FromPrimitive;
use rsnano_core::work::WorkThresholds;
use std::{convert::TryFrom, ffi::CStr, os::raw::c_char, time::Duration};

use rsnano_node::config::{test_node_port, NetworkConstants};

use crate::work::{fill_work_thresholds_dto, WorkThresholdsDto};

#[repr(C)]
pub struct NetworkConstantsDto {
    pub current_network: u16,
    pub work: WorkThresholdsDto,
    pub principal_weight_factor: u32,
    pub default_node_port: u16,
    pub default_rpc_port: u16,
    pub default_ipc_port: u16,
    pub default_websocket_port: u16,
    pub aec_loop_interval_ms: u32,
    pub cleanup_period_s: i64,
    pub keepalive_period_s: i64,
    pub idle_timeout_s: i64,
    pub sync_cookie_cutoff_s: i64,
    pub bootstrap_interval_s: i64,
    pub max_peers_per_ip: usize,
    pub max_peers_per_subnetwork: usize,
    pub peer_dump_interval_s: i64,
    pub protocol_version: u8,
    pub protocol_version_min: u8,
    pub bootstrap_protocol_version_min: u8,
    pub ipv6_subnetwork_prefix_for_limiting: usize,
    pub silent_connection_tolerance_time_s: i64,
    pub vote_broadcast_interval_ms: i64,
    pub telemetry_request_cooldown_ms: i64,
    pub telemetry_request_interval_ms: i64,
    pub telemetry_broadcast_interval_ms: i64,
    pub telemetry_cache_cutoff_ms: i64,
    pub optimistic_activation_delay_s: i64,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_network_constants_create(
    dto: *mut NetworkConstantsDto,
    work: &WorkThresholdsDto,
    network: u16,
) -> i32 {
    let thresholds = WorkThresholds::from(work);
    let network = match FromPrimitive::from_u16(network) {
        Some(n) => n,
        None => return -1,
    };
    let constants = NetworkConstants::new(thresholds, network);
    fill_network_constants_dto(&mut *dto, &constants);
    0
}

pub fn fill_network_constants_dto(dto: &mut NetworkConstantsDto, constants: &NetworkConstants) {
    dto.current_network = constants.current_network as u16;
    fill_work_thresholds_dto(&mut dto.work, &constants.work);
    dto.protocol_version = constants.protocol_version;
    dto.protocol_version_min = constants.protocol_version_min;
    dto.bootstrap_protocol_version_min = constants.bootstrap_protocol_version_min;
    dto.principal_weight_factor = constants.principal_weight_factor;
    dto.default_node_port = constants.default_node_port;
    dto.default_rpc_port = constants.default_rpc_port;
    dto.default_ipc_port = constants.default_ipc_port;
    dto.default_websocket_port = constants.default_websocket_port;
    dto.aec_loop_interval_ms = constants.aec_loop_interval_ms;
    dto.cleanup_period_s = constants.cleanup_period_s;
    dto.keepalive_period_s = constants.keepalive_period.as_secs() as i64;
    dto.idle_timeout_s = constants.idle_timeout_s;
    dto.sync_cookie_cutoff_s = constants.sync_cookie_cutoff_s;
    dto.bootstrap_interval_s = constants.bootstrap_interval_s;
    dto.max_peers_per_ip = constants.max_peers_per_ip;
    dto.max_peers_per_subnetwork = constants.max_peers_per_subnetwork;
    dto.peer_dump_interval_s = constants.peer_dump_interval_s;
    dto.ipv6_subnetwork_prefix_for_limiting = constants.ipv6_subnetwork_prefix_for_limiting;
    dto.silent_connection_tolerance_time_s = constants.silent_connection_tolerance_time_s;
    dto.vote_broadcast_interval_ms = constants.vote_broadcast_interval_ms;
    dto.telemetry_request_cooldown_ms = constants.telemetry_request_cooldown_ms;
    dto.telemetry_request_interval_ms = constants.telemetry_request_interval_ms;
    dto.telemetry_broadcast_interval_ms = constants.telemetry_broadcast_interval_ms;
    dto.telemetry_cache_cutoff_ms = constants.telemetry_cache_cutoff_ms;
    dto.optimistic_activation_delay_s = constants.optimistic_activation_delay.as_secs() as i64;
}

#[no_mangle]
pub extern "C" fn rsn_network_constants_cleanup_period_half_ms(dto: &NetworkConstantsDto) -> i64 {
    NetworkConstants::try_from(dto)
        .unwrap()
        .cleanup_period_half_ms()
}

#[no_mangle]
pub extern "C" fn rsn_network_constants_cleanup_cutoff_s(dto: &NetworkConstantsDto) -> i64 {
    NetworkConstants::try_from(dto).unwrap().cleanup_cutoff_s()
}

#[no_mangle]
pub extern "C" fn rsn_network_constants_is_live_network(dto: &NetworkConstantsDto) -> bool {
    NetworkConstants::try_from(dto).unwrap().is_live_network()
}

#[no_mangle]
pub extern "C" fn rsn_network_constants_is_beta_network(dto: &NetworkConstantsDto) -> bool {
    NetworkConstants::try_from(dto).unwrap().is_beta_network()
}

#[no_mangle]
pub extern "C" fn rsn_network_constants_is_dev_network(dto: &NetworkConstantsDto) -> bool {
    NetworkConstants::try_from(dto).unwrap().is_dev_network()
}

#[no_mangle]
pub extern "C" fn rsn_network_constants_is_test_network(dto: &NetworkConstantsDto) -> bool {
    NetworkConstants::try_from(dto).unwrap().is_test_network()
}

#[no_mangle]
pub extern "C" fn rsn_network_constants_active_network() -> u16 {
    NetworkConstants::active_network() as u16
}

#[no_mangle]
pub extern "C" fn rsn_network_constants_active_network_set(network: u16) {
    if let Some(net) = FromPrimitive::from_u16(network) {
        NetworkConstants::set_active_network(net);
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_network_constants_active_network_set_str(
    network: *const c_char,
) -> i32 {
    let network = CStr::from_ptr(network).to_string_lossy();
    match NetworkConstants::set_active_network_from_str(network) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

#[no_mangle]
pub extern "C" fn rsn_test_node_port() -> u16 {
    test_node_port()
}

impl TryFrom<NetworkConstantsDto> for NetworkConstants {
    type Error = anyhow::Error;

    fn try_from(value: NetworkConstantsDto) -> Result<Self, Self::Error> {
        NetworkConstants::try_from(&value)
    }
}

impl TryFrom<&NetworkConstantsDto> for NetworkConstants {
    type Error = anyhow::Error;

    fn try_from(value: &NetworkConstantsDto) -> Result<Self, Self::Error> {
        Ok(NetworkConstants {
            work: WorkThresholds::from(&value.work),
            principal_weight_factor: value.principal_weight_factor,
            current_network: FromPrimitive::from_u16(value.current_network)
                .ok_or_else(|| anyhow!("invalid current network"))?,
            protocol_version: value.protocol_version,
            protocol_version_min: value.protocol_version_min,
            bootstrap_protocol_version_min: value.bootstrap_protocol_version_min,
            default_node_port: value.default_node_port,
            default_rpc_port: value.default_rpc_port,
            default_ipc_port: value.default_ipc_port,
            default_websocket_port: value.default_websocket_port,
            aec_loop_interval_ms: value.aec_loop_interval_ms,
            cleanup_period_s: value.cleanup_period_s,
            keepalive_period: Duration::from_secs(value.keepalive_period_s as u64),
            idle_timeout_s: value.idle_timeout_s,
            sync_cookie_cutoff_s: value.sync_cookie_cutoff_s,
            bootstrap_interval_s: value.bootstrap_interval_s,
            max_peers_per_ip: value.max_peers_per_ip,
            max_peers_per_subnetwork: value.max_peers_per_subnetwork,
            peer_dump_interval_s: value.peer_dump_interval_s,
            ipv6_subnetwork_prefix_for_limiting: value.ipv6_subnetwork_prefix_for_limiting,
            silent_connection_tolerance_time_s: value.silent_connection_tolerance_time_s,
            vote_broadcast_interval_ms: value.vote_broadcast_interval_ms,
            telemetry_request_cooldown_ms: value.telemetry_request_cooldown_ms,
            telemetry_request_interval_ms: value.telemetry_request_interval_ms,
            telemetry_broadcast_interval_ms: value.telemetry_broadcast_interval_ms,
            telemetry_cache_cutoff_ms: value.telemetry_cache_cutoff_ms,
            optimistic_activation_delay: Duration::from_secs(
                value.optimistic_activation_delay_s as u64,
            ),
        })
    }
}
