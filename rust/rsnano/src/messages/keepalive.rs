use super::{Message, MessageHeader, MessageType};
use crate::{utils::Stream, NetworkConstants};
use anyhow::Result;
use std::{
    any::Any,
    net::{IpAddr, Ipv6Addr, SocketAddr},
};

#[derive(Clone)]
pub struct Keepalive {
    header: MessageHeader,
    peers: [SocketAddr; 8],
}

impl Keepalive {
    pub fn new(constants: &NetworkConstants) -> Self {
        Self {
            header: MessageHeader::new(constants, MessageType::Keepalive),
            peers: empty_peers(),
        }
    }

    pub fn with_version_using(constants: &NetworkConstants, version_using: u8) -> Self {
        Self {
            header: MessageHeader::with_version_using(
                constants,
                MessageType::Keepalive,
                version_using,
            ),
            peers: empty_peers(),
        }
    }

    pub fn with_header(header: &MessageHeader) -> Self {
        Self {
            header: header.clone(),
            peers: empty_peers(),
        }
    }

    pub fn peers(&self) -> &[SocketAddr; 8] {
        &self.peers
    }

    pub fn set_peers(&mut self, peers: &[SocketAddr; 8]) {
        self.peers = *peers;
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        self.header().serialize(stream)?;
        for peer in self.peers() {
            match peer {
                SocketAddr::V4(_) => panic!("ipv6 expected but was ipv4"), //todo make peers IpAddrV6?
                SocketAddr::V6(addr) => {
                    let ip_bytes = addr.ip().octets();
                    stream.write_bytes(&ip_bytes)?;

                    let port_bytes = addr.port().to_le_bytes();
                    stream.write_bytes(&port_bytes)?;
                }
            }
        }
        Ok(())
    }

    pub fn deserialize(&mut self, stream: &mut impl Stream) -> Result<()> {
        debug_assert!(self.header().message_type() == MessageType::Keepalive);

        for i in 0..8 {
            let mut addr_buffer = [0u8; 16];
            let mut port_buffer = [0u8; 2];
            stream.read_bytes(&mut addr_buffer, 16)?;
            stream.read_bytes(&mut port_buffer, 2)?;

            let port = u16::from_le_bytes(port_buffer);
            let ip_addr = Ipv6Addr::from(addr_buffer);

            self.peers[i] = SocketAddr::new(IpAddr::V6(ip_addr), port);
        }
        Ok(())
    }

    pub fn size() -> usize {
        8 * (16 + 2)
    }
}

fn empty_peers() -> [SocketAddr; 8] {
    [SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0); 8]
}

impl Message for Keepalive {
    fn header(&self) -> &MessageHeader {
        &self.header
    }

    fn set_header(&mut self, header: &MessageHeader) {
        self.header = header.clone();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
