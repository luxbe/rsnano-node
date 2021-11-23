use std::os::unix::prelude::OsStrExt;

use crate::config::NodeRpcConfig;

#[repr(C)]
pub struct NodeRpcConfigDto {
    pub rpc_path: [u8; 512],
    pub rpc_path_length: usize,
    pub enable_child_process: bool,
    pub enable_sign_hash: bool,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_node_rpc_config_create(dto: *mut NodeRpcConfigDto) -> i32 {
    let config = match NodeRpcConfig::new() {
        Ok(c) => c,
        Err(_) => return -1,
    };

    let dto = &mut (*dto);
    dto.enable_sign_hash = config.enable_sign_hash;
    dto.enable_child_process = config.child_process.enable;
    let bytes: &[u8] = config.child_process.rpc_path.as_os_str().as_bytes();
    if bytes.len() > dto.rpc_path.len() {
        return -1;
    }
    dto.rpc_path[..bytes.len()].copy_from_slice(bytes);
    dto.rpc_path_length = bytes.len();
    0
}