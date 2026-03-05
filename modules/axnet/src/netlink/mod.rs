mod addr;
mod message;
mod receiver;
mod route;
mod table;

use core::{mem::size_of, task::Context};

use axerrno::{AxError, AxResult, LinuxError, ax_bail};
use axio::prelude::*;
use axpoll::{IoEvents, Pollable};
use bytemuck::Pod;
use enum_dispatch::enum_dispatch;
use spin::RwLock;

pub use self::{
    addr::{GroupIdSet, NetlinkSocketAddr},
    route::RouteTransport,
};
use crate::{
    RecvOptions, SendOptions, Shutdown, SocketAddrEx, SocketOps,
    options::{Configurable, GetSocketOption, SetSocketOption},
};

/// Reads a `Pod` value from a `BufRead` source.
pub(crate) fn read_pod<T: Pod>(reader: &mut impl BufRead) -> AxResult<T> {
    let mut buf = alloc::vec![0u8; size_of::<T>()];
    reader.read_exact(&mut buf)?;
    Ok(bytemuck::pod_read_unaligned(&buf))
}

/// Trait for Netlink transport operations
#[enum_dispatch]
pub trait NetlinkTransportOps: Configurable + Pollable + Send + Sync {
    fn bind(&self, local_addr: &mut NetlinkSocketAddr) -> AxResult;
    fn send(&self, src: impl Read + IoBuf, port: u32, options: SendOptions) -> AxResult<usize>;
    fn recv(&self, dst: impl Write + IoBufMut, options: RecvOptions<'_>) -> AxResult<usize>;
    fn shutdown(&self, _how: Shutdown, _local_addr: Option<&NetlinkSocketAddr>) -> AxResult {
        Ok(())
    }
}

/// Enum for different Netlink transport implementations
#[enum_dispatch(Configurable, NetlinkTransportOps)]
pub enum NetlinkTransport {
    Route(RouteTransport),
    // TODO: more netlink transport support
}

impl Pollable for NetlinkTransport {
    fn poll(&self) -> IoEvents {
        match self {
            NetlinkTransport::Route(route) => route.poll(),
        }
    }

    fn register(&self, context: &mut Context<'_>, events: IoEvents) {
        match self {
            NetlinkTransport::Route(route) => route.register(context, events),
        }
    }
}

/// Netlink socket implementation
pub struct NetlinkSocket {
    transport: NetlinkTransport,
    local_addr: RwLock<Option<NetlinkSocketAddr>>,
    remote_addr: RwLock<Option<NetlinkSocketAddr>>,
}

impl NetlinkSocket {
    /// Creates a new Netlink socket with the specified transport.
    pub fn new(transport: impl Into<NetlinkTransport>) -> Self {
        Self {
            transport: transport.into(),
            local_addr: RwLock::new(None),
            remote_addr: RwLock::new(Some(NetlinkSocketAddr::new_unspecified())),
        }
    }
}

impl Configurable for NetlinkSocket {
    fn get_option_inner(&self, opt: &mut GetSocketOption) -> AxResult<bool> {
        self.transport.get_option_inner(opt)
    }

    fn set_option_inner(&self, opt: SetSocketOption) -> AxResult<bool> {
        self.transport.set_option_inner(opt)
    }
}

impl SocketOps for NetlinkSocket {
    /// Binds the socket to a local Netlink address.
    fn bind(&self, local_addr: SocketAddrEx) -> AxResult {
        let mut local_addr = local_addr.into_netlink()?;
        let mut guard = self.local_addr.write();
        if guard.is_some() {
            ax_bail!(InvalidInput, "already bound");
        }
        self.transport.bind(&mut local_addr)?;
        *guard = Some(local_addr);
        info!("Netlink socket bound to {:?}", local_addr);
        Ok(())
    }

    /// Connects the socket to a remote Netlink address.
    fn connect(&self, remote_addr: SocketAddrEx) -> AxResult {
        // Ensures the socket is bound before connecting.
        if self.local_addr.read().is_none() {
            self.bind(SocketAddrEx::Netlink(NetlinkSocketAddr::new_unspecified()))?;
        }
        let remote_addr = remote_addr.into_netlink()?;
        let mut guard = self.remote_addr.write();
        *guard = Some(remote_addr);
        info!("Netlink socket connected to {:?}", remote_addr);
        Ok(())
    }

    /// Sends data through the Netlink socket.
    fn send(&self, src: impl Read + IoBuf, options: SendOptions) -> AxResult<usize> {
        // Ensure the socket is bound before sending.
        if self.local_addr.read().is_none() {
            self.bind(SocketAddrEx::Netlink(NetlinkSocketAddr::new_unspecified()))?;
        }
        let remote_addr = options.to.clone().map_or_else(
            || {
                // If no remote address is specified, use the connected address.
                self.remote_addr
                    .read()
                    .clone()
                    .ok_or_else(|| AxError::from(LinuxError::EDESTADDRREQ))
            },
            |addr| addr.into_netlink(),
        )?;
        if !remote_addr.is_unspecified() {
            ax_bail!(
                NotConnected,
                "sending netlink route messages to user space is not supported"
            );
        }
        if !options.cmsg.is_empty() {
            ax_bail!(
                InvalidInput,
                "control messages are not supported for netlink sockets"
            );
        }

        self.transport.send(
            src,
            self.local_addr.read().as_ref().unwrap().port(),
            options,
        )
    }

    /// Receives data from the Netlink socket.
    fn recv(&self, dst: impl Write + IoBufMut, options: RecvOptions<'_>) -> AxResult<usize> {
        self.transport.recv(dst, options)
    }

    /// Gets the local address of the Netlink socket.
    fn local_addr(&self) -> AxResult<SocketAddrEx> {
        match self.local_addr.try_read() {
            Some(addr) => addr.map(SocketAddrEx::Netlink).ok_or(AxError::NotConnected),
            None => Err(AxError::NotConnected),
        }
    }

    /// Gets the peer (remote) address of the Netlink socket.
    fn peer_addr(&self) -> AxResult<SocketAddrEx> {
        match self.remote_addr.try_read() {
            Some(addr) => addr.map(SocketAddrEx::Netlink).ok_or(AxError::NotConnected),
            None => Err(AxError::NotConnected),
        }
    }

    /// Shuts down the Netlink socket.
    fn shutdown(&self, how: Shutdown) -> AxResult {
        self.transport
            .shutdown(how, self.local_addr.read().as_ref())
    }
}

impl Pollable for NetlinkSocket {
    fn poll(&self) -> IoEvents {
        if self.local_addr.read().is_none() {
            return IoEvents::empty();
        }
        self.transport.poll()
    }

    fn register(&self, context: &mut Context<'_>, events: IoEvents) {
        self.transport.register(context, events)
    }
}

impl Drop for NetlinkSocket {
    fn drop(&mut self) {
        trace!("Dropping netlink socket");
        self.shutdown(Shutdown::Both).ok();
    }
}
