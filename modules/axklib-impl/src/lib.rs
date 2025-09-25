#![no_std]

use core::time::Duration;

use axklib::{AxResult, IrqHandler, Klib, PhysAddr, VirtAddr, impl_trait};

struct KlibImpl;

impl_trait! {
    impl Klib for KlibImpl {
        fn mem_iomap(addr: PhysAddr, size: usize) -> AxResult<VirtAddr> {
            axmm::iomap(addr, size)
        }

        fn time_busy_wait(dur: Duration) {
            axhal::time::busy_wait(dur);
        }

        fn irq_set_enable(_irq: usize, _enabled: bool) {
            #[cfg(feature = "irq")]
            axhal::irq::set_enable(_irq, _enabled);
            #[cfg(not(feature = "irq"))]
            unimplemented!();
        }

        fn irq_register(_irq: usize, _handler: IrqHandler) -> bool {
            #[cfg(feature = "irq")]
            {
                axhal::irq::register(_irq, _handler)
            }
            #[cfg(not(feature = "irq"))]
            {
                unimplemented!()
            }
        }
    }
}
