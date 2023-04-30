// SPDX-FileCopyrightText: © 2023 Archaic Archea <archaic.archea@gmail.com>
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]
#![feature(
    pointer_byte_offsets,
    sync_unsafe_cell,
    const_mut_refs,
    extend_one,
    thread_local,
    naked_functions,
    asm_const,
    fn_align,
    stdsimd,
    core_intrinsics
)]

extern crate alloc;

pub mod memory;
pub mod uart;
pub mod libs;
pub mod utils;
pub mod traps;
pub mod timing;
pub mod drivers;

pub mod arch;

pub use libs::*;
use spin::Mutex;
use core::sync::atomic::{self, AtomicPtr};

static LOWER_HALF: memory::vmm::Vmm = memory::vmm::Vmm::new("lower_half");

pub static FDT_PTR: Mutex<usize> = Mutex::new(0);
pub static KERN_PHYS: Mutex<usize> = Mutex::new(0);

#[thread_local]
pub static HART_ID: atomic::AtomicUsize = atomic::AtomicUsize::new(1);

pub const IO_OFFSET: u64 = 0xffffffff80000000;

pub fn init(map: &limine::MemoryMap, hhdm_start: u64, hart_id: usize, dtb: *const u8) {
    memory::HHDM_OFFSET.store(hhdm_start, Ordering::Relaxed);
    *FDT_PTR.lock() = dtb as usize;

    unsafe {
        memory::ALLOCATOR.lock().init(memory::HEAP.get() as usize, 16384);
    }
    memory::pmm::init(map);
    memory::init_tls();
    traps::init();
    HART_ID.store(hart_id, core::sync::atomic::Ordering::Relaxed);
    println!("Hart ID: {hart_id}");
    memory::vmm::init();

    unsafe {
        vmem::bootstrap()
    }

    let fdt = unsafe {fdt::Fdt::from_ptr(dtb).unwrap()};
    for node in fdt.all_nodes() {
        if node.name.contains("plic") {
            let ptr = node.reg().unwrap().next().unwrap().starting_address as usize;
            let ptr = ptr + hhdm_start as usize;

            traps::plic::PLIC_ADDR.store(ptr as *mut sifive_plic::Plic, Ordering::Relaxed);
        } else if node.name == "cpus" {
            let tps = node.property("timebase-frequency").unwrap().as_usize().unwrap();
            println!("Clock runs at {}hz", tps);

            timing::TIMER_SPEED.store(tps as u64, Ordering::Relaxed);
        } else if node.name.contains("virtio") {
            let ptr = node.reg().unwrap().next().unwrap().starting_address as u64;
            let ptr = (ptr + IO_OFFSET) as *mut drivers::virtio::VirtIOHeader;
            
            unsafe {
                if (*ptr).is_valid() {
                    if (*ptr).dev_id.read() != drivers::virtio::DeviceType::Reserved {
                        for region in node.reg().unwrap() {
                            println!("Found region {:?}", region);
                        }
                        println!("Found valid VirtIO {:?} device", (*ptr).dev_id.read());
                        drivers::virtio::VIRTIO_LIST.lock().push(AtomicPtr::new(ptr));
                    }
                }
            }
        }
    }

    if traps::plic::PLIC_ADDR.load(Ordering::Relaxed).is_null() {
        panic!("No plic found");
    }

    LOWER_HALF.add(0x1000, (hhdm_start - 0x1001) as usize).unwrap();
    println!("Vmem initialized");

    let claim = LOWER_HALF.alloc(0x8000, vmem::AllocStrategy::InstantFit, true).unwrap();
    println!("Claim 0x{:x}", claim);

    timing::Unit::Seconds(8).wait().unwrap();

    unsafe {
        //use memory::vmm;
        //let level = vmm::LEVELS.load(Ordering::Relaxed);
        //let level = vmm::PageLevel::from_usize(level as usize);

        core::arch::riscv64::wfi();
        //memory::vmm::unmap(vmm::current_table(), memory::VirtualAddress(claim as u64), level, vmm::PageLevel::Level1);
        LOWER_HALF.free(claim, 0x8000);

        core::arch::riscv64::wfi();
    }
}

pub struct IOPtr<T>(*mut T)
    where T: Sized ;

unsafe impl<T> Send for IOPtr<T> {}
unsafe impl<T> Sync for IOPtr<T> {}

impl<T> IOPtr<T> {
    pub const fn new(ptr: *mut T) -> Self {
        Self(ptr)
    }
}

use core::{ops, sync::atomic::Ordering};

impl<T> ops::Deref for IOPtr<T> {
    type Target = T;
    
    fn deref(&self) -> &T {
        unsafe {
            &*self.0
        }
    }
}

impl<T> ops::DerefMut for IOPtr<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe {
            &mut *self.0
        }
    }
}

pub fn current_context() -> usize {
    let id = HART_ID.load(Ordering::Relaxed);

    // Assume we're on qemu
    return 1 + (2 * id);
}