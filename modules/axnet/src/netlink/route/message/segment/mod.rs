pub mod addr;
mod legacy;
pub mod link;

use axerrno::{AxError, AxResult};
use axio::{BufRead, Write};

use self::{addr::AddrSegment, link::LinkSegment};
use crate::netlink::{
    message::{
        ProtocolSegment,
        result::ContinueRead,
        segment::{DoneSegment, ErrorSegment, SegmentHeader, SegmentType},
    },
    read_pod,
};

/// Routing segment enumeration.
#[derive(Debug, Clone)]
pub enum RouteSegment {
    NewLink(LinkSegment),
    GetLink(LinkSegment),
    NewAddr(AddrSegment),
    GetAddr(AddrSegment),
    Done(DoneSegment),
    Error(ErrorSegment),
}

impl ProtocolSegment for RouteSegment {
    fn header(&self) -> &SegmentHeader {
        match self {
            RouteSegment::NewLink(s) | RouteSegment::GetLink(s) => s.header(),
            RouteSegment::NewAddr(s) | RouteSegment::GetAddr(s) => s.header(),
            RouteSegment::Done(s) => s.header(),
            RouteSegment::Error(s) => s.header(),
        }
    }

    fn header_mut(&mut self) -> &mut SegmentHeader {
        match self {
            RouteSegment::NewLink(s) | RouteSegment::GetLink(s) => s.header_mut(),
            RouteSegment::NewAddr(s) | RouteSegment::GetAddr(s) => s.header_mut(),
            RouteSegment::Done(s) => s.header_mut(),
            RouteSegment::Error(s) => s.header_mut(),
        }
    }

    fn read_from(reader: &mut impl BufRead) -> AxResult<ContinueRead<Self, ErrorSegment>> {
        let header = read_pod::<SegmentHeader>(reader)?;

        let segment = match SegmentType::try_from(header.type_) {
            Ok(SegmentType::GETLINK) => {
                LinkSegment::read_from(&header, reader)?.map(RouteSegment::GetLink)
            }
            Ok(SegmentType::GETADDR) => {
                AddrSegment::read_from(&header, reader)?.map(RouteSegment::GetAddr)
            }
            _ => {
                let payload_len = header.padded_payload_len()?;
                reader.consume(payload_len);
                ContinueRead::skipped_with_error(
                    AxError::Unsupported,
                    "the segment type is not supported",
                )
            }
        };

        Ok(segment.map_err(|error| ErrorSegment::new_from_request(&header, Some(error))))
    }

    fn write_to(&self, writer: &mut impl Write) -> AxResult {
        match self {
            RouteSegment::NewLink(s) => s.write_to(writer),
            RouteSegment::NewAddr(s) => s.write_to(writer),
            RouteSegment::Done(s) => s.write_to(writer),
            RouteSegment::Error(s) => s.write_to(writer),
            RouteSegment::GetAddr(_) | RouteSegment::GetLink(_) => {
                unreachable!("kernel should not write get requests to user space");
            }
        }
    }
}
