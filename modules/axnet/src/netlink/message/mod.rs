pub mod attr;
pub mod result;
pub mod segment;

use alloc::vec::Vec;

use axerrno::AxResult;
use axio::{BufRead, Write};

use self::{
    result::ContinueRead,
    segment::{ErrorSegment, SegmentHeader},
};
use crate::netlink::receiver::QueueableMessage;

/// Netlink message with protocol-specific segments.
#[derive(Debug, Clone)]
pub struct Message<T> {
    segments: Vec<T>,
}

impl<T: ProtocolSegment> Message<T> {
    /// Creates a new message with the given segments.
    pub fn new(segments: Vec<T>) -> Self {
        Self { segments }
    }

    /// Writes the message to the given writer.
    pub fn write_to(&self, writer: &mut impl Write) -> AxResult<()> {
        for segment in &self.segments {
            segment.write_to(writer)?;
        }
        Ok(())
    }
}

impl<T: ProtocolSegment> QueueableMessage for Message<T> {
    /// Returns the total length of the message, including all segments.
    fn total_len(&self) -> usize {
        self.segments
            .iter()
            .map(|segment| segment.header().len as usize)
            .sum()
    }
}

/// Protocol-specific segment trait.
pub trait ProtocolSegment: Sized {
    fn header(&self) -> &SegmentHeader;
    fn header_mut(&mut self) -> &mut SegmentHeader;
    fn read_from(reader: &mut impl BufRead) -> AxResult<ContinueRead<Self, ErrorSegment>>;
    fn write_to(&self, writer: &mut impl Write) -> AxResult<()>;
}

/// Alignment for netlink messages.
pub(super) const NLMSG_ALIGN: usize = 4;
