use alloc::collections::BTreeMap;
use core::{
    fmt,
    pin::Pin,
    task::{Context, Poll, Waker},
    time::Duration,
};

use axerrno::AxError;
use axhal::time::{TimeValue, wall_time};
use futures_util::{FutureExt, select_biased};
use kspin::SpinNoIrq;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TimerKey {
    deadline: TimeValue,
    key: u64,
}

enum TimerState {
    Active(Option<Waker>),
    Completed,
}

impl Default for TimerState {
    fn default() -> Self {
        TimerState::Active(None)
    }
}

struct TimerRuntime {
    key: u64,
    wheel: BTreeMap<TimerKey, TimerState>,
}

impl TimerRuntime {
    const fn new() -> Self {
        TimerRuntime {
            key: 0,
            wheel: BTreeMap::new(),
        }
    }

    fn add(&mut self, deadline: TimeValue) -> Option<TimerKey> {
        if deadline <= wall_time() {
            return None;
        }

        let key = TimerKey {
            deadline,
            key: self.key,
        };
        self.wheel.insert(key, TimerState::default());
        self.key += 1;

        Some(key)
    }

    fn update_waker(&mut self, key: &TimerKey, waker: Waker) {
        if let Some(w) = self.wheel.get_mut(key) {
            *w = TimerState::Active(Some(waker));
        }
    }

    fn is_completed(&mut self, key: &TimerKey) -> bool {
        let completed = matches!(self.wheel.get(key), Some(TimerState::Completed));
        if completed {
            self.wheel.remove(key);
        }
        completed
    }

    fn cancel(&mut self, key: &TimerKey) {
        self.wheel.remove(key);
    }

    fn wake(&mut self) {
        if self.wheel.is_empty() {
            return;
        }

        self.wheel
            .iter_mut()
            .take_while(|(k, _)| k.deadline <= wall_time())
            .for_each(|(_, v)| {
                if let TimerState::Active(Some(waker)) =
                    core::mem::replace(v, TimerState::Completed)
                {
                    waker.wake();
                }
            });
    }
}

static TIMER_RUNTIME: SpinNoIrq<TimerRuntime> = SpinNoIrq::new(TimerRuntime::new());

#[allow(dead_code)]
pub(crate) fn check_timer_events() {
    TIMER_RUNTIME.lock().wake();
}

/// Future returned by `sleep` and `sleep_until`.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct TimerFuture(TimerKey);

impl Future for TimerFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut runtime = TIMER_RUNTIME.lock();
        if runtime.is_completed(&self.0) {
            Poll::Ready(())
        } else {
            runtime.update_waker(&self.0, cx.waker().clone());
            Poll::Pending
        }
    }
}

impl Drop for TimerFuture {
    fn drop(&mut self) {
        TIMER_RUNTIME.lock().cancel(&self.0);
    }
}

/// Waits until `duration` has elapsed.
pub async fn sleep(duration: Duration) {
    sleep_until(wall_time() + duration).await
}

/// Waits until `deadline` is reached.
pub async fn sleep_until(deadline: TimeValue) {
    let key = TIMER_RUNTIME.lock().add(deadline);
    if let Some(key) = key {
        TimerFuture(key).await;
    }
}

/// Error returned by [`timeout`] and [`timeout_at`].
#[derive(Debug, PartialEq, Eq)]
pub struct Elapsed(());

impl fmt::Display for Elapsed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "deadline elapsed")
    }
}

impl core::error::Error for Elapsed {}

impl From<Elapsed> for AxError {
    fn from(_: Elapsed) -> Self {
        AxError::TimedOut
    }
}

/// Requires a `Future` to complete before the specified duration has elapsed.
pub async fn timeout<F: IntoFuture>(
    duration: Option<Duration>,
    f: F,
) -> Result<F::Output, Elapsed> {
    timeout_at(
        duration.and_then(|x| x.checked_add(axhal::time::wall_time())),
        f,
    )
    .await
}

/// Requires a `Future` to complete before the specified deadline.
pub async fn timeout_at<F: IntoFuture>(
    deadline: Option<TimeValue>,
    f: F,
) -> Result<F::Output, Elapsed> {
    if let Some(deadline) = deadline {
        select_biased! {
            res = f.into_future().fuse() => Ok(res),
            _ = sleep_until(deadline).fuse() => Err(Elapsed(())),
        }
    } else {
        Ok(f.await)
    }
}
