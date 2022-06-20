mod channel_inproc;
mod channel_tcp;
mod channel_udp;
mod socket;
mod tcp_channels;

pub use channel_inproc::ChannelInProc;
pub use channel_tcp::{ChannelTcp, TcpChannelData};
pub use channel_udp::ChannelUdp;
pub use socket::*;
pub use tcp_channels::TcpChannels;

use crate::Account;

pub trait Channel {
    fn is_temporary(&self) -> bool;
    fn set_temporary(&self, temporary: bool);
    fn get_last_bootstrap_attempt(&self) -> u64;
    fn set_last_bootstrap_attempt(&self, instant: u64);
    fn get_last_packet_received(&self) -> u64;
    fn set_last_packet_received(&self, instant: u64);
    fn get_last_packet_sent(&self) -> u64;
    fn set_last_packet_sent(&self, instant: u64);
    fn get_node_id(&self) -> Option<Account>;
    fn set_node_id(&self, id: Account);
}