//! Interrupt management.

use core::task::Waker;

#[cfg(feature = "ipi")]
pub use axconfig::devices::IPI_IRQ;
use axcpu::trap::{IRQ, register_trap_handler};
#[cfg(feature = "ipi")]
pub use axplat::irq::{IpiTarget, send_ipi};
pub use axplat::irq::{handle, register, set_enable, unregister};
use axpoll::PollSet;

static POLL_TABLE: [PollSet; 0x30] = [const { PollSet::new() }; 0x30];

/// Registers a waker for a IRQ interrupt.
pub fn register_irq_waker(irq: usize, waker: &Waker) {
    set_enable(irq, true);
    POLL_TABLE[irq].register(waker);
}

/// IRQ handler.
///
/// # Warn
///
/// Make sure called in an interrupt context or hypervisor VM exit handler.
#[register_trap_handler(IRQ)]
pub fn irq_handler(vector: usize) -> bool {
    let guard = kernel_guard::NoPreempt::new();
    if let Some(irq) = handle(vector)
        && let Some(set) = POLL_TABLE.get(irq)
    {
        set.wake();
    }
    drop(guard); // rescheduling may occur when preemption is re-enabled.
    true
}
