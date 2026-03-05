mod ack;
mod common;
mod header;

use core::mem::size_of;

use axerrno::{AxError, AxResult};
use axio::{BufRead, Write};
use bytemuck::{Pod, bytes_of};
use memory_addr::align_up;
use num_enum::TryFromPrimitive;

pub use self::{
    ack::{DoneSegment, ErrorSegment},
    common::SegmentCommon,
    header::*,
};
use super::{ContinueRead, NLMSG_ALIGN};
use crate::netlink::read_pod;

pub trait SegmentBody: Sized + Copy + Clone {
    // The actual message body should be `Self::CType`,
    // but older versions of Linux use a legacy type (usually `CRtGenMsg` here).
    // Reference: <https://elixir.bootlin.com/linux/v6.13/source/net/core/rtnetlink.c#L2393>.
    // FIXME: Verify whether the legacy type includes any types other than `CRtGenMsg`.
    type CLegacyType: Pod = Self::CType;
    type CType: Pod + TryInto<Self> + From<Self::CLegacyType> + From<Self>;

    fn read_from(
        header: &SegmentHeader,
        reader: &mut impl BufRead,
    ) -> AxResult<ContinueRead<(Self, usize)>> {
        let mut remaining_len = header.padded_payload_len()?;

        let (c_type, padding_len) = if remaining_len >= size_of::<Self::CType>() {
            let c_type = read_pod::<Self::CType>(reader)?;
            remaining_len -= size_of::<Self::CType>();

            (c_type, Self::padding_len())
        } else if remaining_len >= size_of::<Self::CLegacyType>() {
            let legacy = read_pod::<Self::CLegacyType>(reader)?;
            remaining_len -= size_of::<Self::CLegacyType>();

            (Self::CType::from(legacy), Self::legacy_padding_len())
        } else {
            reader.consume(remaining_len);
            return Ok(ContinueRead::SkippedErr(AxError::InvalidInput));
        };

        let padding_len = padding_len.min(remaining_len);
        reader.consume(padding_len);
        remaining_len -= padding_len;

        match c_type.try_into() {
            Ok(body) => Ok(ContinueRead::Parsed((body, remaining_len))),
            Err(_err) => {
                reader.consume(remaining_len);
                Ok(ContinueRead::SkippedErr(AxError::InvalidInput))
            }
        }
    }

    fn write_to(&self, writer: &mut impl Write) -> AxResult {
        let c_body = Self::CType::from(*self);
        writer.write_all(bytes_of(&c_body))?;

        let padding_len = Self::padding_len();
        if padding_len > 0 {
            writer.write_all(&[0u8; 8][..padding_len])?;
        }

        Ok(())
    }

    fn padding_len() -> usize {
        let payload_len = size_of::<Self::CType>();
        align_up(payload_len, NLMSG_ALIGN) - payload_len
    }

    fn legacy_padding_len() -> usize {
        let payload_len = size_of::<Self::CLegacyType>();
        align_up(payload_len, NLMSG_ALIGN) - payload_len
    }
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive)]
#[expect(clippy::upper_case_acronyms)]
pub enum SegmentType {
    // Standard netlink message types
    NOOP     = 1,
    ERROR    = 2,
    DONE     = 3,
    OVERRUN  = 4,

    // protocol-level types
    NEWLINK  = 16,
    DELLINK  = 17,
    GETLINK  = 18,
    SETLINK  = 19,

    NEWADDR  = 20,
    DELADDR  = 21,
    GETADDR  = 22,

    NEWROUTE = 24,
    DELROUTE = 25,
    GETROUTE = 26,
    // TODO: The list is not exhaustive.
}
