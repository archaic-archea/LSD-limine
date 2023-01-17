#![no_main]
#![no_std]

use core::panic::PanicInfo;

pub extern "C" fn kmain() -> ! {
    putchar('A');
    
    loop {
    }
}

#[panic_handler]
pub fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

/// Not sure how SBI calls work
/// TODO: Figure it out
pub fn putchar(character: char) {
    unsafe {
        core::arch::asm!(
            "li a6, 0",
            "li a7, 1",
            "ecall"
        )
    }
}