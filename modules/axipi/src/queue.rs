use alloc::{collections::VecDeque, sync::Arc};
use core::sync::atomic::AtomicBool;

use crate::event::{Callback, IpiEvent};

/// A queue of IPI events.
///
/// It internally uses a `VecDeque` to store the events, make it
/// possible to pop these events using FIFO order.
pub struct IpiEventQueue {
    events: VecDeque<IpiEvent>,
}

impl IpiEventQueue {
    /// Create a new empty timer list.
    pub fn new() -> Self {
        Self {
            events: VecDeque::new(),
        }
    }

    /// Whether there is no event.
    #[allow(dead_code)]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Push a new event into the queue.
    pub fn push(
        &mut self,
        name: &'static str,
        src_cpu_id: usize,
        callback: Callback,
        done: Option<Arc<AtomicBool>>,
    ) {
        self.events.push_back(IpiEvent {
            name,
            src_cpu_id,
            callback,
            done,
        });
    }

    /// Try to pop the latest event that exists in the queue.
    ///
    /// Return `None` if no event is available.
    #[must_use]
    pub fn pop_one(&mut self) -> Option<(&'static str, usize, Callback, Option<Arc<AtomicBool>>)> {
        if let Some(e) = self.events.pop_front() {
            Some((e.name, e.src_cpu_id, e.callback, e.done))
        } else {
            None
        }
    }
}

impl Default for IpiEventQueue {
    fn default() -> Self {
        Self::new()
    }
}
