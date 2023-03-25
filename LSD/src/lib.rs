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
    extend_one
)]

extern crate alloc;

pub mod memory;
pub mod uart;
pub mod libs;
pub mod utils;

pub use libs::*;

pub static MEM_MAP: limine::LimineMemmapRequest = limine::LimineMemmapRequest::new(0);
pub static HHDM: limine::LimineHhdmRequest = limine::LimineHhdmRequest::new(0);

pub fn init() {
    let hhdm = HHDM.get_response().get().unwrap().offset;
    memory::HHDM_OFFSET.store(hhdm as usize, Ordering::Relaxed);

    unsafe {
        memory::ALLOCATOR.lock().init(memory::HEAP.get() as usize, 16384);
    }

    memory::pmm::init();
    memory::vmm::init();
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