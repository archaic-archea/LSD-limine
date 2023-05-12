#![no_std]
#![no_main]
#![feature(
    naked_functions,
    core_intrinsics
)]

use std::println;

#[no_mangle]
pub extern "C" fn lsd_main(task_id: usize) {
    println!("Task running 0x{:x}", task_id);

    let extend = std::raw_calls::extend_heap(0x1000);

    println!("Extended heap location: {:?}", extend);
    loop {}
}

#[naked]
#[no_mangle]
#[link_section = ".init.entry"]
unsafe extern "C" fn _entry() -> ! {
    #[rustfmt::skip]
    core::arch::asm!("
        .option push
        .option norelax
        lla gp, __global_pointer$
        .option pop

        lla sp, __stack_top

        jal lsd_main

        j 0
        EBREAK
    ", options(noreturn));
}

#[panic_handler]
pub fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}