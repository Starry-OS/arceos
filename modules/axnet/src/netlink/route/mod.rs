pub mod handle;
pub mod message;

use alloc::sync::Arc;
use core::task::Context;

use axerrno::{AxError, AxResult, LinuxError};
use axio::{BufReader, prelude::*};
use axpoll::{IoEvents, PollSet, Pollable};
use axsync::Mutex;
use handle::with_route_table;
use message::{RouteMessage, RouteSegment};

use crate::{
    RecvOptions, SendOptions,
    general::GeneralOptions,
    netlink::{
        NetlinkTransportOps,
        addr::{GroupIdSet, NetlinkSocketAddr},
        message::ProtocolSegment,
        receiver::{MessageQueue, MessageReceiver},
    },
    options::{Configurable, GetSocketOption, SetSocketOption},
};

/// Netlink transport implementation for routing messages
pub struct RouteTransport {
    general: GeneralOptions,
    poller: Arc<PollSet>,
    message_queue: Arc<Mutex<MessageQueue<RouteMessage>>>,
    receiver: MessageReceiver<RouteMessage>,
    groups: GroupIdSet,
}

impl RouteTransport {
    /// Creates a new RouteTransport instance.
    pub fn new() -> Self {
        let poller = Arc::new(PollSet::new());
        let (message_queue, receiver) = MessageQueue::<RouteMessage>::new_pair(poller.clone());
        RouteTransport {
            general: GeneralOptions::default(),
            poller,
            message_queue,
            receiver,
            groups: GroupIdSet::new_empty(),
        }
    }
}

impl Default for RouteTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Configurable for RouteTransport {
    fn get_option_inner(&self, opt: &mut GetSocketOption) -> AxResult<bool> {
        self.general.get_option_inner(opt)
    }

    fn set_option_inner(&self, opt: SetSocketOption) -> AxResult<bool> {
        self.general.set_option_inner(opt)
    }
}

impl NetlinkTransportOps for RouteTransport {
    /// Binds the RouteTransport to the specified local address.
    fn bind(&self, local_addr: &mut NetlinkSocketAddr) -> AxResult {
        local_addr.add_groups(self.groups);
        with_route_table(|table| table.bind(local_addr, self.receiver.clone()))
    }

    /// Sends a RouteMessage to the specified port.
    fn send(&self, src: impl Read + IoBuf, port: u32, _options: SendOptions) -> AxResult<usize> {
        use crate::netlink::message::result::ContinueRead;

        let initial_remaining = src.remaining();
        let mut reader = BufReader::new(src);
        loop {
            let mut segment = match RouteSegment::read_from(&mut reader)? {
                ContinueRead::Parsed(seg) => seg,
                ContinueRead::Skipped => continue,
                ContinueRead::SkippedErr(_error_segment) => {
                    // TODO: Should unicast `_error_segment` as NLMSG_ERROR with original seq/pid to `port` instead of silently continuing.
                    continue;
                }
            };
            let header = segment.header_mut();
            // Set the pid to the sender's port if it's zero
            if header.pid == 0 {
                header.pid = port
            }
            self.handle_request(&segment, port);
            return Ok(initial_remaining);
        }
    }

    /// Receives a RouteMessage from the message queue.
    fn recv(
        &self,
        mut dst: impl Write + IoBufMut,
        mut options: RecvOptions<'_>,
    ) -> AxResult<usize> {
        self.general.recv_poller(self, || {
            let mut message_queue = self.message_queue.lock();
            message_queue.dequeue_if(|msg, len| {
                if dst.remaining_mut() < len {
                    return Err(AxError::from(LinuxError::ENOBUFS));
                }
                trace!("recv message {:?}", msg);
                if let Some(from) = options.from.as_mut() {
                    **from = crate::SocketAddrEx::Netlink(NetlinkSocketAddr::new_unspecified());
                }

                msg.write_to(&mut dst)?;
                Ok((true, len))
            })
        })
    }

    /// Shuts down the RouteTransport, removing any bindings.
    fn shutdown(&self, _how: crate::Shutdown, local_addr: Option<&NetlinkSocketAddr>) -> AxResult {
        with_route_table(|table| {
            if let Some(addr) = local_addr {
                table.unicast_sockets.remove(&addr.port());

                for group_id in addr.groups().ids_iter() {
                    let group = &mut table.multicast_groups[group_id as usize];
                    group.remove_member(addr.port());
                }
            }
            Ok(())
        })
    }
}

impl Pollable for RouteTransport {
    fn poll(&self) -> IoEvents {
        let mut events = IoEvents::OUT;
        let message_queue = self.message_queue.lock();
        events.set(IoEvents::IN, !message_queue.is_empty());
        events
    }

    fn register(&self, context: &mut Context<'_>, events: IoEvents) {
        if events.contains(IoEvents::IN) {
            self.poller.register(context.waker());
        }
    }
}
