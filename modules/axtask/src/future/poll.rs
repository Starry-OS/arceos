use core::{task::Poll, time::Duration};

use axerrno::{AxError, AxResult};
use axpoll::{IoEvents, Pollable};
use futures_util::future::poll_fn;

use super::{block_on, interruptible};

pub struct Poller<'a, P> {
    pollable: &'a P,
    events: IoEvents,
    non_blocking: bool,
    #[cfg(feature = "irq")]
    timeout: Option<Duration>,
}

impl<'a, P: Pollable> Poller<'a, P> {
    pub fn new(pollable: &'a P, events: IoEvents) -> Self {
        Poller {
            pollable,
            events,
            non_blocking: false,
            #[cfg(feature = "irq")]
            timeout: None,
        }
    }

    pub fn non_blocking(mut self, non_blocking: bool) -> Self {
        self.non_blocking = non_blocking;
        self
    }

    #[cfg(feature = "irq")]
    pub fn timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn poll<T>(self, mut f: impl FnMut() -> AxResult<T>) -> AxResult<T> {
        let fut = poll_fn(move |cx| match f() {
            Ok(value) => Poll::Ready(Ok(value)),
            Err(AxError::WouldBlock) => {
                if self.non_blocking {
                    return Poll::Ready(Err(AxError::WouldBlock));
                }
                self.pollable.register(cx, self.events);
                match f() {
                    Ok(value) => Poll::Ready(Ok(value)),
                    Err(AxError::WouldBlock) => Poll::Pending,
                    Err(e) => Poll::Ready(Err(e)),
                }
            }
            Err(e) => Poll::Ready(Err(e)),
        });

        cfg_if::cfg_if! {
            if #[cfg(feature = "irq")] {
                use super::timeout;
                block_on(interruptible(timeout(self.timeout, fut)))??
            } else {
                block_on(interruptible(fut))?
            }
        }
    }
}
