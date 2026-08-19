//! Fault handlers that reboot the nRF52840 into the Adafruit UF2 bootloader.
//!
//! This crate exists purely to replace `panic-probe` in the build. RMK's
//! `#[rmk_central]` / `#[rmk_peripheral]` macros unconditionally emit
//! `use panic_probe as _;`, so we cannot simply drop the dependency: we have
//! to keep the *import name* `panic_probe` alive while swapping the crate
//! behind it. The parent `Cargo.toml` does that with:
//!
//! ```toml
//! panic-probe = { package = "corne-panic-dfu", path = "panic-dfu" }
//! ```
//!
//! Stock `panic-probe` (without `print-defmt`) executes `udf` on panic, which
//! raises a HardFault the CPU sits in forever. A HardFault from the BLE stack
//! or a stack overflow does the same. Either way: no USB, no BLE, no NICENANO
//! drive, until you manually double-tap RST.
//!
//! This crate instead writes `0x57` (`DFU_MAGIC_UF2_RESET`) to
//! `NRF_POWER->GPREGRET` and system-resets, on both Rust panics and HardFault.
//! The Adafruit bootloader then re-enumerates the NICENANO volume.
//!
//! Ref: <https://github.com/adafruit/Adafruit_nRF52_Bootloader/blob/master/src/main.c>

#![no_std]
#![cfg(target_os = "none")]

use core::panic::PanicInfo;

const GPREGRET_ADDR: usize = 0x4000_051C;
const DFU_MAGIC_UF2_RESET: u32 = 0x57;

fn reset_to_uf2() -> ! {
    cortex_m::interrupt::disable();
    unsafe {
        core::ptr::write_volatile(GPREGRET_ADDR as *mut u32, DFU_MAGIC_UF2_RESET);
    }
    cortex_m::peripheral::SCB::sys_reset();
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    reset_to_uf2();
}

#[cortex_m_rt::exception]
unsafe fn HardFault(_ef: &cortex_m_rt::ExceptionFrame) -> ! {
    reset_to_uf2();
}
