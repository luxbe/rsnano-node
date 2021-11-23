use crate::config::RocksDbConfig;

#[repr(C)]
pub struct RocksDbConfigDto {
    pub enable: bool,
    pub memory_multiplier: u8,
    pub io_threads: u32,
}

#[no_mangle]
pub extern "C" fn rsn_using_rocksdb_in_tests() -> bool {
    RocksDbConfig::using_rocksdb_in_tests()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_rocksdb_config_create(dto: *mut RocksDbConfigDto) {
    let config = RocksDbConfig::new();
    let dto = &mut (*dto);
    dto.enable = config.enable;
    dto.memory_multiplier = config.memory_multiplier;
    dto.io_threads = config.io_threads;
}