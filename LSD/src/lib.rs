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
    naked_functions
)]

extern crate alloc;

pub mod memory;
pub mod uart;
pub mod libs;
pub mod utils;

pub use libs::*;
use spin::Mutex;
use core::sync::atomic;

static LOWER_HALF: memory::vmm::Vmm = memory::vmm::Vmm::new("lower_half");

pub static FDT_PTR: Mutex<usize> = Mutex::new(0);
pub static KERN_PHYS: Mutex<usize> = Mutex::new(0);

#[thread_local]
pub static HART_ID: atomic::AtomicUsize = atomic::AtomicUsize::new(1);

pub fn init(map: &limine::MemoryMap, hhdm_start: u64, hart_id: usize) {
    memory::HHDM_OFFSET.store(hhdm_start, Ordering::Relaxed);

    unsafe {
        memory::ALLOCATOR.lock().init(memory::HEAP.get() as usize, 16384);
    }
    memory::pmm::init(map);
    memory::init_tls();
    HART_ID.store(hart_id, core::sync::atomic::Ordering::Relaxed);
    println!("Hart ID: {hart_id}");
    memory::vmm::init();

    unsafe {
        vmem::bootstrap()
    }

    LOWER_HALF.add(0x1000, (hhdm_start - 0x1001) as usize).unwrap();
    println!("Vmem initialized");

    let addr = LOWER_HALF.alloc(0x8000, vmem::AllocStrategy::BestFit).unwrap();
    println!("Address claimed 0x{:x}", addr);
    
    unsafe {
        LOWER_HALF.free(addr, 0x8000);
    }
    let addr = LOWER_HALF.alloc(0x8000, vmem::AllocStrategy::BestFit).unwrap();
    println!("Address claimed 0x{:x}", addr);
    
    unsafe {
        LOWER_HALF.free(addr, 0x8000);
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