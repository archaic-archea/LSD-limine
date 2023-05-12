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

    std::thread::spawn_thread(on_thread);

    println!("Task root running");
    loop {}
}

fn on_thread(task_id: usize, thread_id: usize) {
    println!("Task 0x{:x}: thread 0x{:x} running", task_id, thread_id);
}

#[naked]
#[no_mangle]
unsafe extern "C" fn new_thread() {
    core::arch::asm!(
        "
            li a0, 1
            li a1, 2
            ecall

            beqz a2, 1f
            lla sp, __stack2_top

            j {}

            1:
                mv a1, a3
                mv a0, a2

                li a0, 1
                li a1, 6
                li a2, 2
                li a3, 2
                ecall

                ret
        ", sym on_thread, options(noreturn)
    );
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