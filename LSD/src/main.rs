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

pub static FDT: limine::DtbRequest = limine::DtbRequest::new();
pub static KERN_PHYS: limine::KernelAddressRequest = limine::KernelAddressRequest::new();
pub static HHDM: limine::HhdmRequest = limine::HhdmRequest::new();
pub static SMP: limine::SmpRequest = limine::SmpRequest::new(limine::SmpRequestFlags::empty());
pub static MAP: limine::MemoryMapRequest = limine::MemoryMapRequest::new();
pub static STACK: limine::StackSizeRequest = limine::StackSizeRequest::new(0x0);
pub static PAGING: limine::PagingModeRequest = limine::PagingModeRequest::new(limine::PagingMode::Sv57, limine::PagingModeRequestFlags::empty());

extern "C" fn kmain() -> ! {
    *lsd::FDT_PTR.lock() = FDT.response().unwrap().dtb_ptr as usize;
    *lsd::KERN_PHYS.lock() = KERN_PHYS.response().unwrap().phys;
    lsd::println!("Kernel starting");

    assert!(STACK.has_response(), "Stack request failed");
    assert!(PAGING.has_response(), "Paging request failed");

    lsd::init(MAP.response().unwrap(), HHDM.response().unwrap().base as u64, SMP.response().unwrap().bsp_hartid);

    for core in SMP.response().unwrap().cpus() {
        let new_stack = lsd::memory::pmm::REGION_LIST.lock().claim_frames(0x200).unwrap();

        unsafe {
            core.start(init_cpu, new_stack as usize);
        }
    }

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
        lla sp, __stack_top
        lla gp, __global_pointer$
        .option pop

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

unsafe extern "C" fn init_cpu(info: &limine::SmpInfo) -> ! {
    core::arch::asm!("mv sp, {new_sp}", new_sp = in(reg) info.argument());
    println!("CPU initialized");
    loop {}
}