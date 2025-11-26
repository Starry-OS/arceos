use alloc::boxed::Box;
use core::{
    pin::Pin,
    task::{Context, Waker},
    time::Duration,
};

use axhal::time::{NANOS_PER_MICROS, wall_time, wall_time_nanos};
use axtask::future::sleep_until;
use smoltcp::{
    iface::{Interface, SocketSet},
    time::Instant,
    wire::{HardwareAddress, IpAddress, IpListenEndpoint},
};

use crate::{SOCKET_SET, router::Router};

fn now() -> Instant {
    Instant::from_micros_const((wall_time_nanos() / NANOS_PER_MICROS) as i64)
}

pub struct Service {
    pub iface: Interface,
    router: Router,
    timeout: Option<Pin<Box<dyn Future<Output = ()> + Send>>>,
}
impl Service {
    pub fn new(mut router: Router) -> Self {
        let config = smoltcp::iface::Config::new(HardwareAddress::Ip);
        let iface = Interface::new(config, &mut router, now());

        Self {
            iface,
            router,
            timeout: None,
        }
    }

    pub fn poll(&mut self, sockets: &mut SocketSet) -> bool {
        let timestamp = now();

        self.router.poll(timestamp);
        self.iface.poll(timestamp, &mut self.router, sockets);
        self.router.dispatch(timestamp)
    }

    pub fn get_source_address(&self, dst_addr: &IpAddress) -> IpAddress {
        let Some(rule) = self.router.table.lookup(dst_addr) else {
            panic!("no route to destination: {dst_addr}");
        };
        rule.src
    }

    pub fn device_mask_for(&self, endpoint: &IpListenEndpoint) -> u32 {
        match endpoint.addr {
            Some(addr) => self
                .router
                .table
                .lookup(&addr)
                .map_or(0, |it| 1u32 << it.dev),
            None => u32::MAX,
        }
    }

    pub fn register_waker(&mut self, mask: u32, waker: &Waker) {
        let mut cx = Context::from_waker(waker);
        if let Some(timeout) = &mut self.timeout {
            if timeout.as_mut().poll(&mut cx).is_ready() {
                self.timeout = None;
                waker.wake_by_ref();
            }
        }

        let next = self.iface.poll_at(now(), &mut SOCKET_SET.inner.lock());
        let now = wall_time();
        let delay = next
            .map(|t| {
                let next = Duration::from_micros(t.total_micros() as _);
                next.checked_sub(now)
            })
            .unwrap_or(Some(Duration::from_secs(1)));

        if let Some(delay) = delay {
            for (i, device) in self.router.devices.iter().enumerate() {
                if mask & (1 << i) != 0 {
                    device.register_waker(waker);
                }
            }
            self.timeout = Some(Box::pin(sleep_until(now + delay)));
        } else {
            self.timeout = None;
            waker.wake_by_ref();
        }
    }
}
