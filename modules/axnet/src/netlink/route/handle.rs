use alloc::{boxed::Box, ffi::CString, vec, vec::Vec};
use core::num::{NonZero, NonZeroU32};

use axerrno::{AxError, AxResult, ax_bail};

use crate::{
    device::Device,
    get_service,
    netlink::{
        message::{ProtocolSegment, segment::*},
        route::{RouteTransport, message::*},
        table::{NETLINK_BIND_TABLE, ProtocolBindTable},
    },
};

/// An helper function to access the route protocol bind table.
pub(crate) fn with_route_table<R>(
    f: impl FnOnce(&mut ProtocolBindTable<RouteMessage>) -> AxResult<R>,
) -> AxResult<R> {
    let mut table = NETLINK_BIND_TABLE.route.write();
    f(&mut table)
}

impl RouteTransport {
    /// Handles a GetLink request segment.
    pub fn get_link(request_segment: &LinkSegment) -> AxResult<Vec<RouteSegment>> {
        let filter_by = FilterBy::from_request(request_segment)?;

        // Generate response segments based on the filter.
        let mut response_segments: Vec<RouteSegment> = get_service()
            .iter_devices()
            .filter(|dev| match &filter_by {
                FilterBy::Dump => true,
                FilterBy::Index(index) => dev.get_index() == *index,
                FilterBy::Name(name) => dev.name() == *name,
            })
            .map(|dev| dev_to_new_link(request_segment.header(), dev))
            .map(RouteSegment::NewLink)
            .collect();

        let dump_all = matches!(filter_by, FilterBy::Dump);

        if !dump_all && response_segments.is_empty() {
            ax_bail!(NoSuchDevice, "no matching link found");
        }

        finish_response(request_segment.header(), dump_all, &mut response_segments);
        Ok(response_segments)
    }

    /// Handles a GetAddr request segment.
    pub fn get_addr(request_segment: &AddrSegment) -> AxResult<Vec<RouteSegment>> {
        let dump_all = {
            let flags = GetRequestFlags::from_bits_truncate(request_segment.header().flags);
            flags.contains(GetRequestFlags::DUMP)
        };
        if !dump_all {
            ax_bail!(Unsupported, "GETADDR only supports dump requests");
        }

        let mut response_segments: Vec<RouteSegment> = get_service()
            .iter_devices()
            // Get_addr only support dump, so no filtering needed
            .filter_map(|iface| dev_to_new_addr(request_segment.header(), iface))
            .map(RouteSegment::NewAddr)
            .collect();

        finish_response(request_segment.header(), dump_all, &mut response_segments);

        Ok(response_segments)
    }

    /// Handles a route request segment.
    pub fn handle_request(&self, request: &RouteSegment, dst_port: u32) {
        trace!("Handling request: {:?} for port {}", request, dst_port);
        let response_segments = match request {
            RouteSegment::GetLink(request_segment) => RouteTransport::get_link(request_segment),
            RouteSegment::GetAddr(request_segment) => RouteTransport::get_addr(request_segment),
            _ => Err(AxError::Unsupported),
        };
        trace!("Response segments: {:?}", response_segments);
        let response = match response_segments {
            Ok(segments) => RouteMessage::new(segments),
            Err(_) => {
                // TODO: Future should build RouteSegment::Error(ErrorSegment::new_from_request(...)) and unicast it to `dst_port` instead of returning.
                return;
            }
        };

        if let Err(e) = with_route_table(|table| table.unicast(dst_port, response)) {
            warn!(
                "Failed to unicast netlink response to port {}: {:?}",
                dst_port, e
            );
        }
    }
}

/// Get a new link segment from a device.
fn dev_to_new_link(request_header: &SegmentHeader, dev: &Box<dyn Device>) -> LinkSegment {
    let header = SegmentHeader {
        len: 0,
        type_: SegmentType::NEWLINK as _,
        flags: SegHdrCommonFlags::empty().bits(),
        seq: request_header.seq,
        pid: request_header.pid,
    };

    const AF_UNSPEC: u8 = 0;
    let link_message = LinkSegmentBody {
        family: AF_UNSPEC,
        type_: dev.get_type(),
        index: NonZero::new(dev.get_index()),
        flags: dev.get_flags(),
    };

    let name = CString::new(dev.name()).unwrap_or_default();
    let attrs = vec![
        LinkAttr::Name(name),
        LinkAttr::Mtu(crate::consts::STANDARD_MTU as u32),
    ];

    LinkSegment::new(header, link_message, attrs)
}

/// Get a new address segment from a device.
fn dev_to_new_addr(request_header: &SegmentHeader, dev: &Box<dyn Device>) -> Option<AddrSegment> {
    let ipv4_addr = dev.ipv4_addr()?;
    let prefix_len = dev.prefix_len()?;

    let header = SegmentHeader {
        len: 0,
        type_: SegmentType::NEWADDR as _,
        flags: SegHdrCommonFlags::empty().bits(),
        seq: request_header.seq,
        pid: request_header.pid,
    };

    const AF_INET: u8 = 2;
    let addr_message = AddrSegmentBody {
        family: AF_INET as _,
        prefix_len,
        flags: AddrMessageFlags::PERMANENT,
        scope: RtScope::HOST,
        index: NonZeroU32::new(dev.get_index()),
    };

    let label = CString::new(dev.name()).unwrap_or_default();
    let attrs = vec![
        AddrAttr::Address(ipv4_addr.octets()),
        AddrAttr::Label(label),
        AddrAttr::Local(ipv4_addr.octets()),
    ];

    Some(AddrSegment::new(header, addr_message, attrs))
}

/// Finalizes the response segments.
pub fn finish_response(
    request_header: &SegmentHeader,
    dump_all: bool,
    response_segments: &mut Vec<RouteSegment>,
) {
    if !dump_all {
        debug_assert_eq!(
            response_segments.len(),
            1,
            "non-dump response should have exactly one segment"
        );
        return;
    }
    append_done_segment(request_header, response_segments);
    add_multi_flag(response_segments);
}

/// Appends a done segment as the last segment of the provided segments.
fn append_done_segment(request_header: &SegmentHeader, response_segments: &mut Vec<RouteSegment>) {
    let done_segment = DoneSegment::new_from_request(request_header, None);
    response_segments.push(RouteSegment::Done(done_segment));
}

/// Adds the `MULTI` flag to all segments in `segments`.
fn add_multi_flag(response_segments: &mut [RouteSegment]) {
    for segment in response_segments.iter_mut() {
        let header = segment.header_mut();
        let mut flags = SegHdrCommonFlags::from_bits_truncate(header.flags);
        flags |= SegHdrCommonFlags::MULTI;
        header.flags = flags.bits();
    }
}

/// Filter criteria for GetLink requests.
enum FilterBy<'a> {
    Index(u32),
    Name(&'a str),
    Dump,
}

impl<'a> FilterBy<'a> {
    /// Creates a FilterBy instance from a LinkSegment request.
    fn from_request(segment: &'a LinkSegment) -> AxResult<Self> {
        // Dump has the highest priority.
        if GetRequestFlags::from_bits_truncate(segment.header().flags)
            .contains(GetRequestFlags::DUMP)
        {
            return Ok(Self::Dump);
        }
        if let Some(required_index) = segment.body().index {
            return Ok(Self::Index(required_index.get()));
        }
        let required_name = segment.attrs().iter().find_map(|attr| {
            if let LinkAttr::Name(name) = attr {
                name.to_str().ok()
            } else {
                None
            }
        });
        if let Some(required_name) = required_name {
            return Ok(Self::Name(required_name));
        }

        Err(AxError::InvalidInput)
    }
}
