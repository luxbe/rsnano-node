use super::{Message, MessageHeader, MessageType, MessageVisitor};
use crate::config::NetworkConstants;
use anyhow::Result;
use rsnano_core::utils::Stream;
use std::{any::Any, fmt::Display};

#[derive(Clone)]
pub struct TelemetryReq {
    header: MessageHeader,
}

impl TelemetryReq {
    pub fn new(constants: &NetworkConstants) -> Self {
        Self {
            header: MessageHeader::new(constants, MessageType::TelemetryReq),
        }
    }

    pub fn with_header(header: MessageHeader) -> Self {
        Self { header }
    }

    pub fn deserialize(&mut self, _stream: &mut impl Stream) -> Result<()> {
        debug_assert!(self.header.message_type() == MessageType::TelemetryReq);
        Ok(())
    }
}

impl Message for TelemetryReq {
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

    fn serialize(&self, stream: &mut dyn Stream) -> Result<()> {
        self.header.serialize(stream)
    }

    fn visit(&self, visitor: &mut dyn MessageVisitor) {
        visitor.telemetry_req(self)
    }

    fn clone_box(&self) -> Box<dyn Message> {
        Box::new(self.clone())
    }

    fn message_type(&self) -> MessageType {
        MessageType::TelemetryReq
    }
}

impl Display for TelemetryReq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.header, f)
    }
}
