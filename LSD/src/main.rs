// SPDX-FileCopyrightText: © 2023 Archaic Archea <archaic.archea@gmail.com>
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(naked_functions)]
#![no_std]
#![no_main]

use lsd::println;

pub static INFO: limine::LimineBootInfoRequest = limine::LimineBootInfoRequest::new(0);

extern "C" fn kmain() -> ! {
    let info = INFO.get_response().get().expect("Request for boot info not fulfilled");
    let boot_name = info.name.to_str().unwrap().to_str().unwrap();
    let boot_version = info.version.to_str().unwrap().to_str().unwrap();

    println!("Booting with {} v{}", boot_name, boot_version);

    let mem_map = lsd::MEM_MAP.get_response().get().expect("Request for memory map not fulfilled");
    for entry in mem_map.memmap() {
        println!("Entry found: {:#?}", entry.typ);
        println!("Base found: {:#x}", entry.base);
        println!("Len found: {:#x}", entry.len);
    }

    lsd::init();

    lsd::println!("Kernel end, looping");

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
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("{info:#?}");
    wfi_loop()
}
