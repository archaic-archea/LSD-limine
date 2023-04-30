// SPDX-FileCopyrightText: © 2023 Archaic Archea <archaic.archea@gmail.com>
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(naked_functions, stdsimd)]
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
    
    let root_table = unsafe {&*lsd::memory::vmm::current_table()};
    let table_3 = unsafe {&*root_table.0[0].table()};

    lsd::println!("Kernel end, table dump:\n{root_table:?}\n\n{table_3:?}");

    pause_loop()
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
            println!("Making new upperhalf for new cpu");
            let new_satp = lsd::memory::vmm::new_with_upperhalf() as u64;
            let new_satp_phys = new_satp - lsd::memory::HHDM_OFFSET.load(core::sync::atomic::Ordering::Relaxed);

            let mut satp = lsd::memory::vmm::Satp::new();
            satp.set_ppn(new_satp_phys >> 12);
            satp.set_mode(9);

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
        ld t0, 32(a0)
        ld t1, 8(t0)

        csrw satp, t1

        ld sp, 0(t0)

        lla t0, stvec_trap_shim
        csrw stvec, t0

        lla gp, __global_pointer$
    ");

    lsd::memory::init_tls();
    lsd::traps::init();

    println!("Core 0x{:x} started", smpinfo.hartid);
    lsd::HART_ID.store(smpinfo.hartid, core::sync::atomic::Ordering::Relaxed);

    CORE_INIT.claimed.store(true, core::sync::atomic::Ordering::Relaxed);

    /*Trap on hart 0: TrapFrame {
        sepc: 0xfffffffa80000568,
        registers: GeneralRegisters {
            ra: 0xfffffffa800001d0,
            sp: 0xfffffffa80149f60,
            gp: 0xfffffffa8000dea8,
            tp: 0xffff800080339000,
            t0: 0xdf,
            t1: 0xffffffff90000000,
            t2: 0xfffffffa8000e018,
            s0: 0xfffffffa80012030,
            s1: 0xfffffffa80012030,
            a0: 0x0,
            a1: 0x0,
            a2: 0x0,
            a3: 0xffff8000800a9000,
            a4: 0x80,
            a5: 0xffff8000800a9000,
            a6: 0x1000,
            a7: 0xf0,
            s2: 0xffff80008002a000,
            s3: 0xff,
            s4: 0x1,
            s5: 0xfffffffa8000d14e,
            s6: 0xffff80017fe5b000,
            s7: 0xffff80017fe5c010,
            s8: 0x80025000,
            s9: 0xffff80017fe5c008,
            s10: 0xfffffffa8000e000,
            s11: 0xff,
            t3: 0x0,
            t4: 0xfffffffa800098c2,
            t5: 0x25,
            t6: 0xffffffffffffff8f,
        },
    }
    Cause: StorePageFault
    Stval: 0x0
    fffffffa80000560: 03 39 84 00   ld      s2, 8(s0)
    fffffffa80000564: 03 35 89 00   ld      a0, 8(s2)
    fffffffa80000568: 23 30 05 00   sd      zero, 0(a0)
    */

    loop {
        core::arch::riscv64::pause();
    }
}