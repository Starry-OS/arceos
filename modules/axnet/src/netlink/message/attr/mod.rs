mod noattr;

use alloc::vec::Vec;
use core::mem::size_of;

use axerrno::{AxError, AxResult};
use axio::{BufRead, Write};
use bytemuck::{Pod, Zeroable, bytes_of};
use memory_addr::align_up;
pub use noattr::NoAttr;

use crate::netlink::message::{ContinueRead, NLMSG_ALIGN};

/// Netlink attribute header.
///
/// Reference: <https://elixir.bootlin.com/linux/v6.13/source/include/uapi/linux/netlink.h#L229>.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct AttrHeader {
    len: u16,
    type_: u16,
}

impl AttrHeader {
    /// Creates a new `AttrHeader` from the given type and payload length.
    pub fn from_payload_len(type_: u16, payload_len: usize) -> Self {
        let total_len = payload_len + size_of::<Self>();
        debug_assert!(total_len <= u16::MAX as usize);

        Self {
            len: total_len as u16,
            type_,
        }
    }

    /// Returns the attribute type, masking the nested and net byteorder flags.
    pub fn type_(&self) -> u16 {
        self.type_ & ATTRIBUTE_TYPE_MASK
    }

    /// Returns the length of the attribute payload.
    pub fn payload_len(&self) -> usize {
        self.len as usize - size_of::<Self>()
    }

    /// Returns the total length of the attribute, including header and payload.
    pub fn total_len(&self) -> usize {
        self.len as usize
    }

    /// Returns the total length of the attribute, including padding.
    pub fn total_len_with_padding(&self) -> usize {
        align_up(self.len as usize, NLMSG_ALIGN)
    }

    /// Returns the length of the padding after the attribute payload.
    pub fn padding_len(&self) -> usize {
        self.total_len_with_padding() - self.total_len()
    }
}

const IS_NESTED_MASK: u16 = 1u16 << 15;
const IS_NET_BYTEORDER_MASK: u16 = 1u16 << 14;
const ATTRIBUTE_TYPE_MASK: u16 = !(IS_NESTED_MASK | IS_NET_BYTEORDER_MASK);

/// Trait for Netlink attributes.
pub trait Attribute: Send + Sync {
    /// Returns the attribute type.
    fn type_(&self) -> u16;

    /// Returns the attribute payload as bytes.
    fn payload_as_bytes(&self) -> &[u8];

    /// Returns the total length of the attribute, including padding.
    fn total_len_with_padding(&self) -> usize {
        const DUMMY_TYPE: u16 = 0;

        AttrHeader::from_payload_len(DUMMY_TYPE, self.payload_as_bytes().len())
            .total_len_with_padding()
    }

    /// Reads the attribute from the given header and reader.
    fn read_from(header: &AttrHeader, reader: &mut impl BufRead) -> AxResult<ContinueRead<Self>>
    where
        Self: Sized;

    /// Reads all attributes from the `reader` until `total_len` bytes are read.
    fn read_all_from(
        reader: &mut impl BufRead,
        mut total_len: usize,
    ) -> AxResult<ContinueRead<Vec<Self>>>
    where
        Self: Sized,
    {
        let mut res = Vec::new();

        while total_len > 0 {
            if total_len < size_of::<AttrHeader>() {
                reader.consume(total_len);
                return Ok(ContinueRead::SkippedErr(AxError::InvalidInput));
            }

            let mut buf = [0u8; size_of::<AttrHeader>()];
            reader.read_exact(&mut buf)?;
            let header: AttrHeader = *bytemuck::from_bytes(&buf);
            total_len -= size_of::<AttrHeader>();
            if header.total_len() < size_of::<AttrHeader>() {
                reader.consume(total_len);
                return Ok(ContinueRead::SkippedErr(AxError::InvalidInput));
            }

            if header.payload_len() > total_len {
                reader.consume(total_len);
                return Ok(ContinueRead::SkippedErr(AxError::InvalidInput));
            }
            total_len -= header.payload_len();

            match Self::read_from(&header, reader)? {
                ContinueRead::Parsed(attr) => res.push(attr),
                ContinueRead::Skipped => (),
                ContinueRead::SkippedErr(err) => {
                    reader.consume(total_len);
                    return Ok(ContinueRead::SkippedErr(err));
                }
            }

            let padding_len = total_len.min(header.padding_len());
            reader.consume(padding_len);
            total_len -= padding_len;
        }

        Ok(ContinueRead::Parsed(res))
    }

    /// Writes the attribute to the `writer`.
    fn write_to(&self, writer: &mut impl Write) -> AxResult {
        let type_ = self.type_();
        let payload = self.payload_as_bytes();

        let header = AttrHeader::from_payload_len(type_, payload.len());
        writer.write_all(bytes_of(&header))?;
        writer.write_all(payload)?;

        let padding_len = header.padding_len();
        if padding_len > 0 {
            writer.write_all(&[0u8; 8][..padding_len])?;
        }

        Ok(())
    }
}
