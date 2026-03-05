use alloc::vec;
use core::{net::Ipv4Addr, task::Waker};

use axpoll::PollSet;
use smoltcp::{
    storage::{PacketBuffer, PacketMetadata},
    time::Instant,
    wire::IpAddress,
};

use crate::{
    consts::{SOCKET_BUFFER_SIZE, STANDARD_MTU},
    device::{Device, DeviceFlags, DeviceType},
};

pub struct LoopbackDevice {
    index: u32,
    buffer: PacketBuffer<'static, ()>,
    poll: PollSet,
}
impl LoopbackDevice {
    pub fn new(index: u32) -> Self {
        let buffer = PacketBuffer::new(
            vec![PacketMetadata::EMPTY; SOCKET_BUFFER_SIZE],
            vec![0u8; STANDARD_MTU * SOCKET_BUFFER_SIZE],
        );
        Self {
            index,
            buffer,
            poll: PollSet::new(),
        }
    }
}

impl Device for LoopbackDevice {
    fn name(&self) -> &str {
        "lo"
    }

    fn get_type(&self) -> DeviceType {
        DeviceType::LOOPBACK
    }

    fn get_flags(&self) -> DeviceFlags {
        DeviceFlags::UP | DeviceFlags::LOOPBACK | DeviceFlags::RUNNING
    }

    fn get_index(&self) -> u32 {
        self.index
    }

    fn ipv4_addr(&self) -> Option<Ipv4Addr> {
        Some(Ipv4Addr::new(127, 0, 0, 1))
    }

    fn prefix_len(&self) -> Option<u8> {
        Some(8)
    }

    fn recv(&mut self, buffer: &mut PacketBuffer<()>, _timestamp: Instant) -> bool {
        self.buffer.dequeue().ok().is_some_and(|(_, rx_buf)| {
            buffer
                .enqueue(rx_buf.len(), ())
                .unwrap()
                .copy_from_slice(rx_buf);
            true
        })
    }

    fn send(&mut self, next_hop: IpAddress, packet: &[u8], _timestamp: Instant) -> bool {
        match self.buffer.enqueue(packet.len(), ()) {
            Ok(tx_buf) => {
                tx_buf.copy_from_slice(packet);
                self.poll.wake();
                true
            }
            Err(_) => {
                warn!(
                    "Loopback device buffer is full, dropping packet to {}",
                    next_hop
                );
                false
            }
        }
    }

    fn register_waker(&self, waker: &Waker) {
        self.poll.register(waker);
    }
}
