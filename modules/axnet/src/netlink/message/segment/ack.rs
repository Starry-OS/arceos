use alloc::vec::Vec;

use axerrno::AxError;
use bytemuck::{Pod, Zeroable};

use crate::netlink::message::{
    attr::NoAttr,
    segment::{SegHdrCommonFlags, SegmentBody, SegmentCommon, SegmentHeader, SegmentType},
};

/// Acknowledgment segment without attributes.
pub type DoneSegment = SegmentCommon<DoneSegmentBody, NoAttr>;

/// Body of a Done segment.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DoneSegmentBody {
    error_code: i32,
}

impl SegmentBody for DoneSegmentBody {
    type CType = DoneSegmentBody;
}

impl DoneSegment {
    /// Creates a new Done segment from the given request header and optional error.
    pub fn new_from_request(request_header: &SegmentHeader, error: Option<AxError>) -> Self {
        let header = SegmentHeader {
            len: 0,
            type_: SegmentType::DONE as _,
            flags: SegHdrCommonFlags::empty().bits(),
            seq: request_header.seq,
            pid: request_header.pid,
        };

        let body = {
            let error_code = error.map_or(0, |e| -(e.code() as i32));
            DoneSegmentBody { error_code }
        };

        Self::new(header, body, Vec::new())
    }
}

/// Error segment without attributes.
pub type ErrorSegment = SegmentCommon<ErrorSegmentBody, NoAttr>;

/// Body of an Error segment.
#[repr(C)]
#[derive(Debug, Pod, Clone, Copy, Zeroable)]
pub struct ErrorSegmentBody {
    error_code: i32,
    request_header: SegmentHeader,
}

impl SegmentBody for ErrorSegmentBody {
    type CType = ErrorSegmentBody;
}

impl ErrorSegment {
    /// Creates a new Error segment from the given request header and optional error.
    pub fn new_from_request(request_header: &SegmentHeader, error: Option<AxError>) -> Self {
        let header = SegmentHeader {
            len: 0,
            type_: SegmentType::ERROR as _,
            flags: SegHdrCommonFlags::empty().bits(),
            seq: request_header.seq,
            pid: request_header.pid,
        };

        let body = {
            let error_code = error.map_or(0, |e| -(e.code() as i32));
            ErrorSegmentBody {
                error_code,
                request_header: *request_header,
            }
        };

        Self::new(header, body, Vec::new())
    }
}
