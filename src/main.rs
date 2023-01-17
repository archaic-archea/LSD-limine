#![no_main]
#![no_std]

use core::panic::PanicInfo;

pub extern "C" fn kmain() -> ! {
    sbi::legacy::console_putchar(b'A');

    loop {
    }
}

#[panic_handler]
pub fn panic(_info: &PanicInfo) -> ! {
    loop {}
}