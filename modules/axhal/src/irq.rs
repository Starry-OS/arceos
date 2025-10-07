//! Interrupt management.

use core::task::Waker;

use axcpu::trap::{IRQ, register_trap_handler};
use axpoll::PollSet;

pub use axplat::irq::{handle, register, set_enable, unregister};

#[cfg(feature = "ipi")]
pub use axplat::irq::{IpiTarget, send_ipi};

#[cfg(feature = "ipi")]
pub use axconfig::devices::IPI_IRQ;

/// IRQ handler.
///
/// # Warn
///
/// Make sure called in an interrupt context or hypervisor VM exit handler.
#[register_trap_handler(IRQ)]
pub fn irq_handler(vector: usize) -> bool {
    let guard = kernel_guard::NoPreempt::new();
    handle(vector);
    drop(guard); // rescheduling may occur when preemption is re-enabled.
    true
}

static POLL_SET_TABLE: [PollSet; 0x30] = [const { PollSet::new() }; 0x30];

fn poll_handler(irq: usize) {
    POLL_SET_TABLE[irq].wake();
}

/// Registers a waker for a IRQ interrupt.
pub fn register_irq_waker(irq: u32, waker: &Waker) {
    POLL_SET_TABLE[irq as usize].register(waker);
    axplat::irq::register(irq as usize, poll_handler);
}
