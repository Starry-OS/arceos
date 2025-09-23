#![no_std]

use core::time::Duration;

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

        fn irq_set_enable(_irq: usize, _enabled: bool) {
            #[cfg(feature = "irq")]
            irq::set_enable(_irq, _enabled);
            #[cfg(not(feature = "irq"))]
            unimplemented!();
        }

        fn irq_register(_irq: usize, _handler: IrqHandler) -> bool {
            #[cfg(feature = "irq")]
            {
                irq::register(_irq, _handler)
            }
            #[cfg(not(feature = "irq"))]
            {
                unimplemented!()
            }
        }
    }
}
