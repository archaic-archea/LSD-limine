// SPDX-FileCopyrightText: © 2023 Archaic Archea <archaic.archea@gmail.com>
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(naked_functions, stdsimd, generic_arg_infer)]
#![no_std]
#![no_main]

use lsd::println;

pub static FDT: limine::DtbRequest = limine::DtbRequest::new();
pub static KERN_PHYS: limine::KernelAddressRequest = limine::KernelAddressRequest::new();
pub static HHDM: limine::HhdmRequest = limine::HhdmRequest::new();
pub static SMP: limine::SmpRequest = limine::SmpRequest::new(limine::SmpRequestFlags::empty());
pub static MAP: limine::MemoryMapRequest = limine::MemoryMapRequest::new();
pub static PAGING: limine::PagingModeRequest = limine::PagingModeRequest::new(limine::PagingMode::Sv57, limine::PagingModeRequestFlags::empty());

#[repr(C)]
struct CoreInit {
    sp: usize,
    satp: usize,
    claimed: core::sync::atomic::AtomicBool
}

static mut CORE_INIT: CoreInit = CoreInit {sp: 0, satp: 0, claimed: core::sync::atomic::AtomicBool::new(false)};

extern "C" fn kmain() -> ! {
    *lsd::FDT_PTR.lock() = FDT.response().unwrap().dtb_ptr as usize;
    *lsd::KERN_PHYS.lock() = KERN_PHYS.response().unwrap().phys;
    lsd::println!("Kernel starting");

    assert!(PAGING.has_response(), "Paging request failed");

    unsafe {
        lsd::init(
            MAP.response().unwrap(), 
            HHDM.response().unwrap().base as u64, 
            SMP.response().unwrap().bsp_hartid, 
            FDT.response().unwrap().dtb_ptr
        );
    }

    smp_init();

    // Make it so we'll jump to user mode on an `sret`
    let mut sstatus = lsd::arch::regs::Sstatus::new();
    sstatus.set_spp(false);
    sstatus.set_spie(true);
    unsafe {
        sstatus.set();
    }
    
    use lsd::traps::task::Privilege;

    unsafe {lsd::userspace::init_task_queues()};
    let task = lsd::userspace::load(lsd::USER_PROG, Privilege::Root);
    println!("Loaded user program task with id 0x{:x}", task.task_id);
    lsd::traps::task::new_task(task);
    lsd::timing::Unit::MilliSeconds(10).set().unwrap();
    lsd::userspace::start_tasks();

    // If we get here, thats bad, very bad

    unreachable!("How the hell did we get here?");
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

        lla sp, __stack_top
        lla t0, stvec_trap_shim
        csrw stvec, t0

        2:
            j {}
    ", sym kmain, options(noreturn));
}

fn pause_loop() -> ! {
    loop {
        core::arch::riscv64::pause();
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("{info:#?}");
    pause_loop()
}

fn smp_init() {
    for core in SMP.response().unwrap().cpus() {
        let hart_id = core.hartid;
        if hart_id != SMP.response().unwrap().bsp_hartid {
            let satp = lsd::memory::vmm::Satp::new();

            unsafe {
                CORE_INIT.satp = core::mem::transmute(satp);
                CORE_INIT.sp = lsd::memory::pmm::REGION_LIST.lock().claim_continuous(0x80).unwrap() as usize;

                println!("Core 0x{:x} starting", hart_id);
                core.start(core_main, core::ptr::addr_of!(CORE_INIT) as usize);

                while !CORE_INIT.claimed.load(core::sync::atomic::Ordering::Relaxed) {
                    core::arch::riscv64::pause();
                }
    
                CORE_INIT.claimed.store(true, core::sync::atomic::Ordering::Relaxed);
            }
        }
    }
}

/// # Safety
/// Should only be called once per core
#[no_mangle]
pub unsafe extern "C" fn core_main(smpinfo: &limine::SmpInfo) -> ! {
    core::arch::asm!("
        .option push
        .option norelax
        lla gp, __global_pointer$
        .option pop

        ld t0, 32(a0)
        ld t1, 8(t0)

        csrw satp, t1
        sfence.vma

        ld sp, 0(t0)

        lla t0, stvec_trap_shim
        csrw stvec, t0
    ");

    lsd::memory::init_tls();
    lsd::traps::init();

    println!("Core 0x{:x} started", smpinfo.hartid);
    lsd::HART_ID.store(smpinfo.hartid, core::sync::atomic::Ordering::Relaxed);

    CORE_INIT.claimed.store(true, core::sync::atomic::Ordering::Relaxed);

    lsd::userspace::init_task_queues();
    lsd::userspace::start_tasks();

    unreachable!("How the hell did we get here");
}