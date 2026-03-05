use bytemuck::{Pod, Zeroable};

use super::{addr::CIfaddrMsg, link::CIfinfoMsg};

/// `rtgenmsg` in Linux.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CRtGenMsg {
    pub family: u8,
}

impl From<CRtGenMsg> for CIfinfoMsg {
    fn from(value: CRtGenMsg) -> Self {
        Self {
            family: value.family,
            _pad: 0,
            type_: 0,
            index: 0,
            flags: 0,
            change: 0,
        }
    }
}

impl From<CRtGenMsg> for CIfaddrMsg {
    fn from(value: CRtGenMsg) -> Self {
        Self {
            family: value.family,
            prefix_len: 0,
            flags: 0,
            scope: 0,
            index: 0,
        }
    }
}
