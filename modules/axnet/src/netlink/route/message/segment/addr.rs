use core::num::NonZeroU32;

use axerrno::AxError;
use bitflags::bitflags;
use bytemuck::{Pod, Zeroable};
use num_enum::TryFromPrimitive;

use super::legacy::CRtGenMsg;
use crate::netlink::{
    message::segment::{SegmentBody, SegmentCommon},
    route::message::AddrAttr,
};

/// Address segment type.
pub type AddrSegment = SegmentCommon<AddrSegmentBody, AddrAttr>;

impl SegmentBody for AddrSegmentBody {
    type CLegacyType = CRtGenMsg;
    type CType = CIfaddrMsg;
}

/// `ifaddrmsg` in Linux.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CIfaddrMsg {
    pub family: u8,
    pub prefix_len: u8,
    pub flags: u8,
    pub scope: u8,
    pub index: u32,
}

/// Address segment body.
#[derive(Debug, Clone, Copy)]
pub struct AddrSegmentBody {
    pub family: i32,
    pub prefix_len: u8,
    pub flags: AddrMessageFlags,
    pub scope: RtScope,
    pub index: Option<NonZeroU32>,
}

impl TryFrom<CIfaddrMsg> for AddrSegmentBody {
    type Error = AxError;

    fn try_from(value: CIfaddrMsg) -> Result<Self, Self::Error> {
        let flags = AddrMessageFlags::from_bits_truncate(value.flags as u32);
        let scope = RtScope::try_from(value.scope).map_err(|_| AxError::InvalidInput)?;
        let index = NonZeroU32::new(value.index);

        Ok(Self {
            family: value.family as i32,
            prefix_len: value.prefix_len,
            flags,
            scope,
            index,
        })
    }
}

impl From<AddrSegmentBody> for CIfaddrMsg {
    fn from(value: AddrSegmentBody) -> Self {
        let index = if let Some(index) = value.index {
            index.get()
        } else {
            0
        };
        CIfaddrMsg {
            family: value.family as u8,
            prefix_len: value.prefix_len,
            flags: value.flags.bits() as u8,
            scope: value.scope as _,
            index,
        }
    }
}

bitflags! {
    /// Flags for address messages.
    #[derive(Debug, Clone, Copy)]
    pub struct AddrMessageFlags: u32 {
        const SECONDARY      = 0x01;
        const NODAD          = 0x02;
        const OPTIMISTIC     = 0x04;
        const DADFAILED      = 0x08;
        const HOMEADDRESS    = 0x10;
        const DEPRECATED	 = 0x20;
        const TENTATIVE		 = 0x40;
        const PERMANENT		 = 0x80;
        const MANAGETEMPADDR = 0x100;
        const NOPREFIXROUTE	 = 0x200;
        const MCAUTOJOIN	 = 0x400;
        const STABLE_PRIVACY = 0x800;
    }
}

/// Route scope.
#[repr(u8)]
#[derive(Debug, Clone, Copy, TryFromPrimitive)]
#[allow(non_camel_case_types)]
pub enum RtScope {
    UNIVERSE = 0,
    SITE     = 200,
    LINK     = 253,
    HOST     = 254,
    NOWHERE  = 255,
}
