use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    unsafe extern "Rust" {
        pub fn __arceos_panic_handler(info: &core::panic::PanicInfo) -> !;
    }
    unsafe {
        __arceos_panic_handler(info);
    }
}

/// The default panic handler for ArceOS based kernels.
///
/// The user can override it by defining their own panic handler with the macro
/// `#[axmacros::panic_handler]`.
#[linkage = "weak"]
fn __arceos_panic_handler(info: &PanicInfo) -> ! {
    ax_println!("{}", info);
    axhal::power::system_off()
}
