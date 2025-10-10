//! [ArceOS](https://github.com/arceos-org/arceos) memory management module.

#![no_std]

#[macro_use]
extern crate log;

extern crate alloc;

mod aspace;
pub mod backend;
mod page_iter;

use axerrno::{AxError, LinuxResult};
use axhal::{
    mem::{MemRegionFlags, phys_to_virt},
    paging::MappingFlags,
};
use kspin::SpinNoIrq;
use lazyinit::LazyInit;
use memory_addr::{MemoryAddr, PhysAddr, VirtAddr, va};
use memory_set::MappingError;

pub use self::aspace::AddrSpace;

static KERNEL_ASPACE: LazyInit<SpinNoIrq<AddrSpace>> = LazyInit::new();

fn mapping_to_ax_error(err: MappingError) -> AxError {
    if !matches!(err, MappingError::AlreadyExists) {
        warn!("Mapping error: {err:?}");
    }
    match err {
        MappingError::InvalidParam => AxError::InvalidInput,
        MappingError::AlreadyExists => AxError::AlreadyExists,
        MappingError::BadState => AxError::BadState,
    }
}

fn reg_flag_to_map_flag(f: MemRegionFlags) -> MappingFlags {
    let mut ret = MappingFlags::empty();
    if f.contains(MemRegionFlags::READ) {
        ret |= MappingFlags::READ;
    }
    if f.contains(MemRegionFlags::WRITE) {
        ret |= MappingFlags::WRITE;
    }
    if f.contains(MemRegionFlags::EXECUTE) {
        ret |= MappingFlags::EXECUTE;
    }
    if f.contains(MemRegionFlags::DEVICE) {
        ret |= MappingFlags::DEVICE;
    }
    if f.contains(MemRegionFlags::UNCACHED) {
        ret |= MappingFlags::UNCACHED;
    }
    ret
}

/// Creates a new address space for kernel itself.
pub fn new_kernel_aspace() -> LinuxResult<AddrSpace> {
    let mut aspace = AddrSpace::new_empty(
        va!(axconfig::plat::KERNEL_ASPACE_BASE),
        axconfig::plat::KERNEL_ASPACE_SIZE,
    )?;
    for r in axhal::mem::memory_regions() {
        // mapped range should contain the whole region if it is not aligned.
        let start = r.paddr.align_down_4k();
        let end = (r.paddr + r.size).align_up_4k();
        aspace.map_linear(
            phys_to_virt(start),
            start,
            end - start,
            reg_flag_to_map_flag(r.flags),
        )?;
    }
    Ok(aspace)
}

/// Returns the globally unique kernel address space.
pub fn kernel_aspace() -> &'static SpinNoIrq<AddrSpace> {
    &KERNEL_ASPACE
}

/// Returns the root physical address of the kernel page table.
pub fn kernel_page_table_root() -> PhysAddr {
    KERNEL_ASPACE.lock().page_table_root()
}

/// Initializes virtual memory management.
///
/// It mainly sets up the kernel virtual memory address space and recreate a
/// fine-grained kernel page table.
pub fn init_memory_management() {
    info!("Initialize virtual memory management...");

    let kernel_aspace = new_kernel_aspace().expect("failed to initialize kernel address space");
    debug!("kernel address space init OK: {:#x?}", kernel_aspace);
    KERNEL_ASPACE.init_once(SpinNoIrq::new(kernel_aspace));
    unsafe { axhal::asm::write_kernel_page_table(kernel_page_table_root()) };
}

/// Initializes kernel paging for secondary CPUs.
pub fn init_memory_management_secondary() {
    unsafe { axhal::asm::write_kernel_page_table(kernel_page_table_root()) };
}

/// Maps a physical memory region to virtual address space for device access.
pub fn iomap(addr: PhysAddr, size: usize) -> Result<VirtAddr, AxError> {
    let virt = phys_to_virt(addr);

    let virt_aligned = virt.align_down_4k();
    let addr_aligned = addr.align_down_4k();
    let size_aligned = (addr + size).align_up_4k() - addr_aligned;

    let flags = MappingFlags::DEVICE | MappingFlags::READ | MappingFlags::WRITE;
    let mut tb = kernel_aspace().lock();
    match tb.map_linear(virt_aligned, addr_aligned, size_aligned, flags) {
        Err(AxError::AlreadyExists) => {}
        Err(e) => {
            return Err(e);
        }
        Ok(_) => {}
    }
    // flush TLB
    tb.protect(virt_aligned, size_aligned, flags)?;
    Ok(virt)
}
