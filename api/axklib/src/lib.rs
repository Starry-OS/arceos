//! axklib — small kernel-helper abstractions used across the microkernel
//!
//! This crate exposes a tiny, no_std-compatible trait (`Klib`) that the
//! platform/board layer must implement. The trait provides a handful of
//! common kernel helpers such as memory mapping helpers, timing utilities,
//! and IRQ registration. The implementation is supplied by the platform
//! (see `modules/axklib-impl`) and consumed by drivers and other modules.
//!
//! The crate also provides small convenience modules (`mem`, `time`, `irq`)
//! that re-export the trait methods with shorter names to make call sites
//! more ergonomic.
//!
//! Example usage:
//!
//! ```ignore
//! // map 4K of device MMIO at physical address `paddr`
//! let vaddr = axklib::mem::iomap(paddr, 0x1000)?;
//!
//! // busy-wait for 100 microseconds
//! axklib::time::busy_wait(core::time::Duration::from_micros(100));
//!
//! // register an IRQ handler
//! axklib::irq::register(32, my_irq_handler);
//! ```

#![no_std]
// #![allow(missing_docs)]

use core::time::Duration;

pub use axerrno::AxError;
/// Type alias for a simple IRQ handler function pointer.
///
/// Handlers use the raw ABI required by the platform and take no arguments.
pub type IrqHandler = fn(usize);

pub use memory_addr::{PhysAddr, VirtAddr};

use trait_ffi::*;

/// The kernel helper trait that platform implementations must provide.
#[def_extern_trait]
pub trait Klib {
    /// Map a physical memory region into the kernel's virtual address space.
    ///
    /// Parameters:
    /// - `addr`: The physical start address of the region to map.
    /// - `size`: The size in bytes of the region to map. Typically page-aligned.
    ///
    /// Returns:
    /// - `Ok(VirtAddr)` with the virtual address corresponding to the mapped
    ///   physical region on success.
    /// - `Err(_)` with an `AxResult` error code on failure.
    ///
    /// Notes:
    /// - The returned `VirtAddr` is page-aligned when the underlying mapping
    ///   mechanism requires it.
    /// - The actual mapping behavior is platform-specific; callers should
    ///   treat this as an allocation-like operation and ensure the mapping
    ///   is later cleaned up if the platform/ABI requires it.
    fn mem_iomap(addr: PhysAddr, size: usize) -> Result<VirtAddr, AxError>;

    /// Busy-wait the current execution context for the provided duration.
    ///
    /// This is intended for short delays where sleeping or timer-based
    /// suspension is not available or not appropriate (for example, very
    /// early boot or simple spin-waits). Implementations should aim to be
    /// reasonably accurate for small durations but exact timing guarantees
    /// are platform-dependent.
    fn time_busy_wait(dur: Duration);

    /// Enable or disable the edge/level for a platform IRQ.
    ///
    /// `irq` is a platform IRQ number. `enabled` selects whether the IRQ
    /// should be enabled (true) or disabled (false).
    fn irq_set_enable(irq: usize, enabled: bool);

    /// Register a simple IRQ handler for the given IRQ number.
    ///
    /// Returns `true` if the handler was successfully registered, `false`
    /// otherwise. The exact semantics (e.g. whether multiple handlers are
    /// allowed) are platform-specific; callers should consult the platform
    /// implementation.
    fn irq_register(irq: usize, handler: IrqHandler) -> bool;
}

/// Convenience re-export for memory IO mapping.
pub mod mem {
    pub use super::klib::mem_iomap as iomap;
}

/// Convenience re-export for busy-wait timing.
pub mod time {
    pub use super::klib::time_busy_wait as busy_wait;
}

/// Convenience re-exports for IRQ operations.
pub mod irq {
    pub use super::klib::irq_register as register;
    pub use super::klib::irq_set_enable as set_enable;
}
