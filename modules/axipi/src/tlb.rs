use axhal::mem::VirtAddr;
use axtask::{TaskExt, current};
use page_table_multiarch::TlbFlushIf;

use crate::{
    MulticastCallback, run_on_bitmask_except_self, run_on_each_cpu_except_self,
    secondary_cpus_ready,
};

struct TlbFlushImpl;

#[crate_interface::impl_interface]
impl TlbFlushIf for TlbFlushImpl {
    fn flush_all(vaddr: Option<VirtAddr>) {
        if axconfig::plat::CPU_NUM == 1 || !secondary_cpus_ready() {
            // local
            axhal::asm::flush_tlb(None);
        } else {
            let callback = MulticastCallback::new(move || {
                axhal::asm::flush_tlb(vaddr);
            });
            if let Some(ext) = current().task_ext() {
                let on_cpu_mask = ext.on_cpu_mask();
                run_on_bitmask_except_self("flush", callback, on_cpu_mask, true);
            } else {
                run_on_each_cpu_except_self("flush", callback, true);
            }
        }
    }
}
