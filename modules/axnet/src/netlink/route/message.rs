mod attr;
mod segment;

pub use attr::{addr::AddrAttr, link::LinkAttr};
pub use segment::{
    RouteSegment,
    addr::{AddrMessageFlags, AddrSegment, AddrSegmentBody, RtScope},
    link::{LinkSegment, LinkSegmentBody},
};

/// Route message type.
pub type RouteMessage = crate::netlink::message::Message<RouteSegment>;
