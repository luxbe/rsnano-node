mod json;
mod output_tracker;
mod output_tracker_mt;

use std::time::{Duration, SystemTime};

pub use json::*;
pub use output_tracker::{OutputListener, OutputTracker};
pub use output_tracker_mt::{OutputListenerMt, OutputTrackerMt};

mod stream;
pub use stream::*;

mod toml;
pub use toml::*;

mod logger;
pub use logger::{ConsoleLogger, Logger, NullLogger};

mod container_info;
pub use container_info::{ContainerInfo, ContainerInfoComponent};

pub trait Serialize {
    fn serialized_size() -> usize;
    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()>;
}

pub trait Deserialize {
    type Target;
    fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<Self::Target>;
}

impl Serialize for u64 {
    fn serialized_size() -> usize {
        std::mem::size_of::<u64>()
    }

    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        stream.write_u64_be(*self)
    }
}

impl Deserialize for u64 {
    type Target = Self;
    fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<u64> {
        stream.read_u64_be()
    }
}

impl Serialize for [u8; 64] {
    fn serialized_size() -> usize {
        64
    }

    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        stream.write_bytes(self)
    }
}

impl Deserialize for [u8; 64] {
    type Target = Self;

    fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<Self::Target> {
        let mut buffer = [0; 64];
        stream.read_bytes(&mut buffer, 64)?;
        Ok(buffer)
    }
}

pub fn get_cpu_count() -> usize {
    // Try to read overridden value from environment variable
    let value = std::env::var("NANO_HARDWARE_CONCURRENCY")
        .unwrap_or_else(|_| "0".into())
        .parse::<usize>()
        .unwrap_or_default();

    if value > 0 {
        return value;
    }

    //todo: use std::thread::available_concurrency once it's in stable
    if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
        cpuinfo.match_indices("processor").count()
    } else {
        1
    }
}

pub type MemoryIntensiveInstrumentationCallback = extern "C" fn() -> bool;

pub static mut MEMORY_INTENSIVE_INSTRUMENTATION: Option<MemoryIntensiveInstrumentationCallback> =
    None;

extern "C" fn default_is_sanitizer_build_callback() -> bool {
    false
}
pub static mut IS_SANITIZER_BUILD: MemoryIntensiveInstrumentationCallback =
    default_is_sanitizer_build_callback;

pub fn memory_intensive_instrumentation() -> bool {
    unsafe {
        match MEMORY_INTENSIVE_INSTRUMENTATION {
            Some(f) => f(),
            None => false,
        }
    }
}

pub fn is_sanitizer_build() -> bool {
    unsafe { IS_SANITIZER_BUILD() }
}

pub fn nano_seconds_since_epoch() -> u64 {
    system_time_as_nanoseconds(SystemTime::now())
}

pub fn seconds_since_epoch() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn system_time_from_nanoseconds(nanos: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_nanos(nanos)
}

pub fn system_time_as_nanoseconds(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .expect("Time went backwards")
        .as_nanos() as u64
}

pub fn get_env_or_default<T>(variable_name: &str, default: T) -> T
where
    T: core::str::FromStr + Copy,
{
    std::env::var(variable_name)
        .map(|v| v.parse::<T>().unwrap_or(default))
        .unwrap_or(default)
}

pub fn get_env_or_default_string(variable_name: &str, default: impl Into<String>) -> String {
    std::env::var(variable_name).unwrap_or_else(|_| default.into())
}

pub trait Latch: Send + Sync {
    fn wait(&self);
}

pub struct NullLatch {}

impl NullLatch {
    pub fn new() -> Self {
        Self {}
    }
}

impl Latch for NullLatch {
    fn wait(&self) {}
}
