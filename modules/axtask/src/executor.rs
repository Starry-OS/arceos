use alloc::{boxed::Box, collections::VecDeque, sync::Arc, task::Wake};
use core::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
};

use kernel_guard::NoPreemptIrqSave;
use kspin::SpinNoIrq;
use lazyinit::LazyInit;

use crate::{current_run_queue, select_run_queue, TaskId, WeakAxTaskRef};

pub struct AxExecutor {
    queue: SpinNoIrq<VecDeque<Arc<AsyncTask>>>,
}

impl AxExecutor {
    pub fn new() -> Self {
        Self {
            queue: SpinNoIrq::new(VecDeque::new()),
        }
    }

    pub fn add_task(&self, task: Arc<AsyncTask>) {
        self.queue.lock().push_back(task);
    }

    pub fn pop_task(&self) -> Option<Arc<AsyncTask>> {
        self.queue.lock().pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.lock().is_empty()
    }
}

impl Default for AxExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[percpu::def_percpu]
static READY_QUEUE: LazyInit<Arc<AxExecutor>> = LazyInit::new();

#[percpu::def_percpu]
static WAKE_COUNT: AtomicUsize = AtomicUsize::new(0);

#[percpu::def_percpu]
static BLOCKED_TASK: SpinNoIrq<Option<WeakAxTaskRef>> = SpinNoIrq::new(None);

pub(crate) fn init() {
    READY_QUEUE.with_current(|q| {
        q.init_once(Arc::new(AxExecutor::new()));
    });
}

pub fn set_blocked_task(task: WeakAxTaskRef) {
    BLOCKED_TASK.with_current(|t| {
        *t.lock() = Some(task);
    });
}

pub fn clear_blocked_task() {
    BLOCKED_TASK.with_current(|t| {
        *t.lock() = None;
    });
}

fn wake_blocked_task() {
    BLOCKED_TASK.with_current(|t| {
        if let Some(weak) = t.lock().as_ref() {
            if let Some(task) = weak.upgrade() {
                select_run_queue::<NoPreemptIrqSave>(&task).unblock_task(task, false);
            }
        }
    });
}

/// An asynchronous task that wraps a future.
pub struct AsyncTask {
    id: TaskId,
    future: SpinNoIrq<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>,
    executor: Arc<AxExecutor>,
    enqueued: AtomicBool,
}

impl AsyncTask {
    pub fn new(
        future: impl Future<Output = ()> + Send + 'static,
        executor: Arc<AxExecutor>,
    ) -> Arc<Self> {
        Arc::new(Self {
            id: TaskId::new(),
            future: SpinNoIrq::new(Box::pin(future)),
            executor,
            enqueued: AtomicBool::new(false),
        })
    }

    pub fn id(&self) -> TaskId {
        self.id
    }

    pub(crate) fn poll(self: &Arc<Self>) -> Poll<()> {
        self.enqueued.store(false, Ordering::Release);
        let waker = Waker::from(self.clone());
        let mut cx = Context::from_waker(&waker);
        let mut future = self.future.lock();
        future.as_mut().poll(&mut cx)
    }

    fn enqueue(self: &Arc<Self>) -> bool {
        if self
            .enqueued
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            self.executor.add_task(self.clone());
            true
        } else {
            false
        }
    }
}

impl Wake for AsyncTask {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        if self.enqueue() {
            WAKE_COUNT.with_current(|c| c.fetch_add(1, Ordering::Release));
            wake_blocked_task();
        }
    }
}

pub fn spawn<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    let executor = READY_QUEUE.with_current(|q| q.clone());
    let task = AsyncTask::new(future, executor.clone());
    if task.enqueue() {
        WAKE_COUNT.with_current(|c| c.fetch_add(1, Ordering::Release));
    }
}

pub fn run_once() -> Option<Poll<()>> {
    if let Some(task) = READY_QUEUE.with_current(|q| q.pop_task()) {
        Some(task.poll())
    } else {
        None
    }
}

pub fn run_for(max_steps: usize) -> bool {
    let mut ran = false;
    for _ in 0..max_steps {
        if run_once().is_none() {
            break;
        }
        ran = true;
    }
    ran
}

pub fn wake_count() -> usize {
    WAKE_COUNT.with_current(|c| c.load(Ordering::Acquire))
}

pub fn is_empty() -> bool {
    READY_QUEUE.with_current(|q| q.is_empty())
}

pub fn run_until_idle() {
    const SPIN_LIMIT: usize = 64;
    let mut spin_count = 0;
    loop {
        let seen = wake_count();

        while run_once().is_some() {}

        let done = {
            let _guard = kernel_guard::NoPreempt::new();
            is_empty() && wake_count() == seen
        };

        if done {
            break;
        }

        if spin_count < SPIN_LIMIT {
            spin_count += 1;
            core::hint::spin_loop();
        } else {
            spin_count = 0;
            current_run_queue::<NoPreemptIrqSave>().yield_current();
        }
    }
}
