#![no_std]

use core::time::Duration;

use axklib::_if::Klib;
use axklib::*;

struct KlibImpl;

impl_trait! {
    impl Klib for KlibImpl {
        fn mem_iomap(addr: PhysAddr, size: usize) -> AxResult<VirtAddr> {
            mem::iomap(addr, size)
        }

        fn time_busy_wait(dur: Duration) {
            time::busy_wait(dur);
        }

        #[cfg(feature = "irq")]
        fn irq_set_enable(irq: usize, enabled: bool) {
            irq::set_enable(irq, enabled);
        }

        #[cfg(feature = "irq")]
        fn irq_register(irq: usize, handler: IrqHandler) -> bool {
            irq::register(irq, handler)
        }
    }
}
