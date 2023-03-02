#![feature(naked_functions)]
#![no_std]
#![no_main]

extern "C" fn kmain(hart_id: usize, _fdt_ptr: *const u8) -> ! {
    let uart_data = 0x1000_0000 as *mut u8;

    if hart_id == 0 {
        for c in b"We're hart 0!\n" {
            unsafe { uart_data.write_volatile(*c) };
        }
    }

    for c in b"Hello, world!\n" {
        unsafe { uart_data.write_volatile(*c) };
    }

    wfi_loop()
}

#[naked]
#[no_mangle]
#[link_section = ".init.boot"]
unsafe extern "C" fn _boot() -> ! {
    #[rustfmt::skip]
    core::arch::asm!("
        csrw sie, zero
        csrci sstatus, 2
        
        .option push
        .option norelax
        lla gp, __global_pointer$
        .option pop

        lla sp, __tmp_stack_top

        lla t0, __bss_start
        lla t1, __bss_end

        1:
            beq t0, t1, 2f
            sd zero, (t0)
            addi t0, t0, 8
            j 1b

        2:
            j {}
    ", sym kmain, options(noreturn));
}

fn wfi_loop() -> ! {
    loop {
        unsafe { core::arch::asm!("wfi") };
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    wfi_loop()
}
