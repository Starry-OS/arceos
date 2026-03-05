use alloc::vec::Vec;
use core::mem::size_of;

use axerrno::AxResult;
use axio::{BufRead, Write};
use bytemuck::bytes_of;

use crate::netlink::message::{
    attr::Attribute,
    result::ContinueRead,
    segment::{SegmentBody, SegmentHeader},
};

/// Common segment structure with body and attributes.
#[derive(Debug, Clone)]
pub struct SegmentCommon<Body, Attr> {
    header: SegmentHeader,
    body: Body,
    attrs: Vec<Attr>,
}

impl<Body, Attr> SegmentCommon<Body, Attr> {
    pub const HEADER_LEN: usize = size_of::<SegmentHeader>();

    /// Returns a reference to the segment header.
    pub fn header(&self) -> &SegmentHeader {
        &self.header
    }

    /// Returns a mutable reference to the segment header.
    pub fn header_mut(&mut self) -> &mut SegmentHeader {
        &mut self.header
    }

    /// Returns a reference to the segment body.
    pub fn body(&self) -> &Body {
        &self.body
    }

    /// Returns a reference to the segment attributes.
    pub fn attrs(&self) -> &[Attr] {
        &self.attrs
    }
}

impl<Body: SegmentBody, Attr: Attribute> SegmentCommon<Body, Attr> {
    pub const BODY_LEN: usize = size_of::<Body::CType>();

    /// Creates a new segment with the given header, body, and attributes.
    pub fn new(header: SegmentHeader, body: Body, attrs: Vec<Attr>) -> Self {
        let mut res = Self {
            header,
            body,
            attrs,
        };
        res.header.len = res.total_len() as u32;
        res
    }

    /// Reads a segment from the given header and reader.
    pub fn read_from(
        header: &SegmentHeader,
        reader: &mut impl BufRead,
    ) -> AxResult<ContinueRead<Self>> {
        let (body, remain_len) = match Body::read_from(header, reader)? {
            ContinueRead::Parsed(parsed) => parsed,
            ContinueRead::Skipped => return Ok(ContinueRead::Skipped),
            ContinueRead::SkippedErr(err) => return Ok(ContinueRead::SkippedErr(err)),
        };

        let attrs = match Attr::read_all_from(reader, remain_len)? {
            ContinueRead::Parsed(attrs) => attrs,
            ContinueRead::Skipped => Vec::new(),
            ContinueRead::SkippedErr(err) => return Ok(ContinueRead::SkippedErr(err)),
        };

        Ok(ContinueRead::Parsed(Self {
            header: *header,
            body,
            attrs,
        }))
    }

    /// Writes the segment to the given writer.
    pub fn write_to(&self, writer: &mut impl Write) -> AxResult {
        writer.write_all(bytes_of(&self.header))?;
        self.body.write_to(writer)?;
        for attr in &self.attrs {
            attr.write_to(writer)?;
        }
        Ok(())
    }

    /// Returns the total length of the segment, including header, body, and attributes.
    pub fn total_len(&self) -> usize {
        Self::HEADER_LEN + Self::BODY_LEN + self.attrs_len()
    }
}

impl<Body, Attr: Attribute> SegmentCommon<Body, Attr> {
    /// Returns the total length of the segment attributes, including padding.
    pub fn attrs_len(&self) -> usize {
        self.attrs
            .iter()
            .map(|attr| attr.total_len_with_padding())
            .sum()
    }
}
