use core::num::NonZeroU32;

use axerrno::AxError;
use bytemuck::{Pod, Zeroable};

use super::legacy::CRtGenMsg;
use crate::{
    device::{DeviceFlags, DeviceType},
    netlink::{
        message::segment::{SegmentBody, SegmentCommon},
        route::message::LinkAttr,
    },
};

pub type LinkSegment = SegmentCommon<LinkSegmentBody, LinkAttr>;

impl SegmentBody for LinkSegmentBody {
    type CLegacyType = CRtGenMsg;
    type CType = CIfinfoMsg;
}
/// `ifinfomsg` in Linux.
///
/// Reference: <https://elixir.bootlin.com/linux/v6.13/source/include/uapi/linux/rtnetlink.h#L561>.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CIfinfoMsg {
    /// AF_UNSPEC
    pub family: u8,
    /// Padding byte
    pub _pad: u8,
    /// Device type
    pub type_: u16,
    /// Interface index
    pub index: u32,
    /// Device flags
    pub flags: u32,
    /// Change mask
    pub change: u32,
}

/// Link segment body.
#[derive(Debug, Clone, Copy)]
pub struct LinkSegmentBody {
    pub family: u8,
    pub type_: DeviceType,
    pub index: Option<NonZeroU32>,
    pub flags: DeviceFlags,
}

impl TryFrom<CIfinfoMsg> for LinkSegmentBody {
    type Error = AxError;

    fn try_from(value: CIfinfoMsg) -> Result<Self, Self::Error> {
        let family = value.family;
        let type_ = DeviceType::try_from(value.type_).map_err(|_| AxError::InvalidInput)?;
        let index = NonZeroU32::new(value.index);
        let flags = DeviceFlags::from_bits_truncate(value.flags);

        Ok(Self {
            family,
            type_,
            index,
            flags,
        })
    }
}

impl From<LinkSegmentBody> for CIfinfoMsg {
    fn from(value: LinkSegmentBody) -> Self {
        CIfinfoMsg {
            family: value.family,
            _pad: 0,
            type_: value.type_ as _,
            index: value.index.map(NonZeroU32::get).unwrap_or(0),
            flags: value.flags.bits(),
            change: 0,
        }
    }
}
