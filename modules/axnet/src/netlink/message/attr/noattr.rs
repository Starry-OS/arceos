use alloc::vec::Vec;

use axerrno::AxResult;
use axio::BufRead;

use super::{Attribute, AttrHeader};
use crate::netlink::message::ContinueRead;

/// An attribute type that represents no attribute.
#[derive(Debug, Clone)]
pub enum NoAttr {}

impl Attribute for NoAttr {
    fn type_(&self) -> u16 {
        match *self {}
    }

    fn payload_as_bytes(&self) -> &[u8] {
        match *self {}
    }

    fn read_from(header: &AttrHeader, reader: &mut impl BufRead) -> AxResult<ContinueRead<Self>>
    where
        Self: Sized,
    {
        let payload_len = header.payload_len();
        reader.consume(payload_len);

        Ok(ContinueRead::Skipped)
    }

    fn read_all_from(
        reader: &mut impl BufRead,
        total_len: usize,
    ) -> AxResult<ContinueRead<Vec<Self>>>
    where
        Self: Sized,
    {
        reader.consume(total_len);

        Ok(ContinueRead::Skipped)
    }
}
