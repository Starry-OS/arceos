//! Interrupt management.

use core::sync::atomic::{AtomicPtr, Ordering};

#[cfg(feature = "ipi")]
pub use axconfig::devices::IPI_IRQ;
use axcpu::trap::{IRQ, register_trap_handler};
#[cfg(feature = "ipi")]
pub use axplat::irq::{IpiTarget, send_ipi};
pub use axplat::irq::{handle, register, set_enable, unregister};

// TODO: design a better api!
static IRQ_HOOK: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());

pub fn register_irq_hook(hook: fn(usize)) -> bool {
    IRQ_HOOK
        .compare_exchange(
            core::ptr::null_mut(),
            hook as *mut (),
            Ordering::SeqCst,
            Ordering::SeqCst,
        )
        .is_ok()
}

/// IRQ handler.
///
/// # Warn
///
/// Make sure called in an interrupt context or hypervisor VM exit handler.
#[register_trap_handler(IRQ)]
pub fn irq_handler(vector: usize) -> bool {
    let guard = kernel_guard::NoPreempt::new();
    if let Some(irq) = handle(vector) {
        let hook = IRQ_HOOK.load(Ordering::SeqCst);
        if !hook.is_null() {
            let hook = unsafe { core::mem::transmute::<*mut (), fn(usize)>(hook) };
            hook(irq);
        }
    }
    drop(guard); // rescheduling may occur when preemption is re-enabled.
    true
}
