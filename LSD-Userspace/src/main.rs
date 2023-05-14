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

    let join_handle = std::thread::spawn_thread(thread);
    let join_handle = std::thread::spawn_thread(thread);
    let join_handle = std::thread::spawn_thread(thread);
    let _ = join_handle.join();
}

fn thread(task_id: usize, thread_id: usize) {
    println!("New thread running task 0x{:x} thread 0x{:x}", task_id, thread_id);
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

        jal lsd_main

        j 0
        EBREAK
    ", options(noreturn));
}

#[panic_handler]
pub fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}