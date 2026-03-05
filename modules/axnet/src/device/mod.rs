use core::task::Waker;

use bitflags::bitflags;
use num_enum::TryFromPrimitive;
use smoltcp::{
    storage::PacketBuffer,
    time::Instant,
    wire::{IpAddress, Ipv4Address},
};

mod ethernet;
mod loopback;
#[cfg(feature = "vsock")]
mod vsock;

pub use ethernet::*;
pub use loopback::*;
#[cfg(feature = "vsock")]
pub use vsock::*;

pub trait Device: Send + Sync {
    fn name(&self) -> &str;

    fn get_type(&self) -> DeviceType;

    fn get_flags(&self) -> DeviceFlags;

    fn get_index(&self) -> u32;

    fn ipv4_addr(&self) -> Option<Ipv4Address>;

    fn prefix_len(&self) -> Option<u8>;

    fn recv(&mut self, buffer: &mut PacketBuffer<()>, timestamp: Instant) -> bool;
    /// Sends a packet to the next hop.
    ///
    /// Returns `true` if this operation resulted in the readiness of receive
    /// operation. This is true for loopback devices and can be used to speed
    /// up packet processing.
    fn send(&mut self, next_hop: IpAddress, packet: &[u8], timestamp: Instant) -> bool;

    fn register_waker(&self, waker: &Waker);
}

/// Device type.
///
/// Reference: <https://elixir.bootlin.com/linux/v6.0.18/source/include/uapi/linux/if_arp.h#L30>
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[allow(dead_code)]
pub enum DeviceType {
    // Arp protocol hardware identifiers
    /// from KA9Q: NET/ROM pseudo
    NETROM    = 0,
    /// Ethernet 10Mbps
    ETHER     = 1,
    /// Experimental Ethernet
    EETHER    = 2,

    // Dummy types for non ARP hardware
    /// IPIP tunnel
    TUNNEL    = 768,
    /// IP6IP6 tunnel
    TUNNEL6   = 769,
    /// Frame Relay Access Device
    FRAD      = 770,
    /// SKIP vif
    SKIP      = 771,
    /// Loopback device
    LOOPBACK  = 772,
    /// Localtalk device
    LOCALTALK = 773,
    // TODO: This enum is not exhaustive
}

bitflags! {
    /// Device flags.
    ///
    /// Reference: <https://elixir.bootlin.com/linux/v6.0.18/source/include/uapi/linux/if.h#L82>
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DeviceFlags: u32 {
        /// Device is up
        const UP				= 1<<0;
        /// Broadcast address valid
        const BROADCAST			= 1<<1;
        /// Turn on debugging
        const DEBUG			    = 1<<2;
        /// Loopback net
        const LOOPBACK			= 1<<3;
        /// Device is has p-p link
        const POINTOPOINT		= 1<<4;
        /// Avoid use of trailers
        const NOTRAILERS		= 1<<5;
        /// Device RFC2863 OPER_UP
        const RUNNING			= 1<<6;
        /// No ARP protocol
        const NOARP			    = 1<<7;
        /// Receive all packets
        const PROMISC			= 1<<8;
        /// Receive all multicast packets
        const ALLMULTI			= 1<<9;
        /// Master of a load balancer
        const MASTER			= 1<<10;
        /// Slave of a load balancer
        const SLAVE			    = 1<<11;
        /// Supports multicast
        const MULTICAST			= 1<<12;
        /// Can set media type
        const PORTSEL			= 1<<13;
        /// Auto media select active
        const AUTOMEDIA			= 1<<14;
        /// Dialup device with changing addresses
        const DYNAMIC			= 1<<15;
        /// Driver signals L1 up
        const LOWER_UP			= 1<<16;
        /// Driver signals dormant
        const DORMANT			= 1<<17;
        /// Echo sent packets
        const ECHO			    = 1<<18;
    }
}
