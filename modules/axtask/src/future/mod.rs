//! Future support.

use alloc::{sync::Arc, task::Wake};
use axerrno::AxError;
use core::{
    fmt,
    future::poll_fn,
    pin::pin,
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, Poll, Waker},
};

use kernel_guard::NoPreemptIrqSave;

use crate::{
    executor,
    AxTaskRef, WeakAxTaskRef, current, current_run_queue, select_run_queue,
};

mod poll;
pub use poll::*;

mod time;
pub use time::*;

struct AxWaker {
    task: WeakAxTaskRef,
    woke: AtomicBool,
}

impl AxWaker {
    fn new(task: &AxTaskRef) -> Arc<Self> {
        Arc::new(AxWaker {
            task: Arc::downgrade(task),
            woke: AtomicBool::new(false),
        })
    }
}

impl Wake for AxWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        if let Some(task) = self.task.upgrade() {
            self.woke.store(true, Ordering::Release);
            select_run_queue::<NoPreemptIrqSave>(&task).unblock_task(task, false);
        }
    }
}

/// Blocks the current task until the given future is resolved.
///
/// While waiting for the main future, this function also drives other async tasks
/// in the per-CPU executor. The thread only blocks when both:
/// - The main future is pending (not yet ready)
/// - The per-CPU executor has no ready tasks to run
///
/// When async tasks in the executor are woken, they will also wake up this
/// blocked thread, ensuring that the executor continues to make progress.
pub fn block_on<F: IntoFuture>(f: F) -> F::Output {
    let mut fut = pin!(f.into_future());

    let curr = current();
    // It's necessary to keep a strong reference to the current task
    // to prevent it from being dropped while blocking.
    let task = curr.clone();

    let waker = AxWaker::new(&task);
    let woke = &waker.woke;
    let waker = Waker::from(waker.clone());
    let mut cx = Context::from_waker(&waker);

    loop {
        woke.store(false, Ordering::Release);

        if let Poll::Ready(output) = fut.as_mut().poll(&mut cx) {
            return output;
        }

        // While waiting for the main future, this function also drives other async tasks
        loop {
            let seen = executor::wake_count();

            while executor::run_once().is_some() {}

            let should_block = {
                let _guard = kernel_guard::NoPreempt::new();
                executor::is_empty()
                    && executor::wake_count() == seen
                    && !woke.load(Ordering::Acquire)
            };

            if should_block {
                executor::set_blocked_task(Arc::downgrade(&task));
                current_run_queue::<NoPreemptIrqSave>().blocked_resched();
                executor::clear_blocked_task();
                break;
            } else if woke.load(Ordering::Acquire) {
                break;
            }
        }
    }
}

/// Error returned by [`interruptible`].
#[derive(Debug, PartialEq, Eq)]
pub struct Interrupted;

impl fmt::Display for Interrupted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "interrupted")
    }
}

impl core::error::Error for Interrupted {}

impl From<Interrupted> for AxError {
    fn from(_: Interrupted) -> Self {
        AxError::Interrupted
    }
}

/// Makes a future interruptible.
pub async fn interruptible<F: IntoFuture>(f: F) -> Result<F::Output, Interrupted> {
    let mut f = pin!(f.into_future());
    let curr = current();
    poll_fn(|cx| {
        if curr.poll_interrupt(cx).is_ready() {
            return Poll::Ready(Err(Interrupted));
        }
        f.as_mut().poll(cx).map(Ok)
    })
    .await
}
