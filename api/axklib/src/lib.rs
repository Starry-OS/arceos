#![no_std]

use core::time::Duration;

pub use axerrno::AxResult;
pub type IrqHandler = fn();
pub use memory_addr::{PhysAddr, VirtAddr};

use trait_ffi::*;

#[def_extern_trait]
pub trait Klib {
    /// Maps a physical memory region to a virtual address space and returns the virtual address.
    ///
    /// The returned virtual address is guaranteed to be page-aligned.
    fn mem_iomap(addr: PhysAddr, size: usize) -> AxResult<VirtAddr>;

    fn time_busy_wait(dur: Duration);

    #[cfg(feature = "irq")]
    fn irq_set_enable(irq: usize, enabled: bool);
    #[cfg(feature = "irq")]
    fn irq_register(irq: usize, handler: IrqHandler) -> bool;
}

pub mod mem {
    pub use super::klib::mem_iomap as iomap;
}

pub mod time {
    pub use super::klib::time_busy_wait as busy_wait;
}

#[cfg(feature = "irq")]
pub mod irq {
    pub use super::klib::irq_register as register;
    pub use super::klib::irq_set_enable as set_enable;
}
