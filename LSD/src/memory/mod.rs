// SPDX-FileCopyrightText: © 2023 Archaic Archea <archaic.archea@gmail.com>
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::{sync::atomic::{Ordering, AtomicUsize}, cell::SyncUnsafeCell};
use linked_list::LinkedListAllocator;

pub mod pmm;
pub mod vmm;
pub mod linked_list;

pub static HEAP: SyncUnsafeCell<[u8; 16384]> = SyncUnsafeCell::new([0; 16384]);

#[global_allocator]
pub static ALLOCATOR: Locked<LinkedListAllocator> = Locked::new(LinkedListAllocator::new());

pub static HHDM_OFFSET: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug)]
pub struct PhysicalAddress(usize);

impl PhysicalAddress {
    pub const fn null() -> Self {
        Self(0)
    }

    pub fn as_ptr(&self) -> *mut u8 {
        (self.0 + HHDM_OFFSET.load(Ordering::Relaxed)) as *mut u8
    }

    pub fn from_ptr(ptr: *mut u8) -> Self {
        Self(ptr as usize - HHDM_OFFSET.load(Ordering::Relaxed))
    }

    pub fn add(&self, rhs: usize) -> Self {
        Self(self.0 + rhs)
    }

    pub fn new(val: usize) -> Self {
        Self(val)
    }

    pub fn index(&self, idx: usize) -> usize {
        let shift = 12 + ((idx - 1) * 9);

        if idx == 0 {
            return self.0 & 0xfff;
        } else if idx > 5 {
            panic!("Index {idx} too high, valid indexes: 0-5")
        } else {
            return (self.0 >> shift) & 0x1ff;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtualAddress(usize);

impl VirtualAddress {
    pub const fn null() -> Self {
        Self(0)
    }

    pub fn from_ptr(ptr: *mut u8) -> Self {
        Self(ptr as usize)
    }

    pub fn as_ptr(&self) -> *mut u8 {
        self.0 as *mut u8
    }

    pub fn add(&self, rhs: usize) -> Self {
        Self(self.0 + rhs)
    }

    pub fn new(val: usize) -> Self {
        Self(val)
    }

    pub fn index(&self, idx: usize) -> usize {
        let shift = 12 + ((idx - 1) * 9);

        if idx == 0 {
            return self.0 & 0xfff;
        } else if idx > 5 {
            panic!("Index {idx} too high, valid indexes: 0-5")
        } else {
            return (self.0 >> shift) & 0x1ff;
        }
    }

    pub fn no_offset(&self) -> Self {
        Self(self.0 & (!0xfff))
    }

    /// Returns a version of self that can generate branches and not juts pages
    pub fn can_branch(&self) -> Self {
        Self(self.0 & (!0x1fffff))
    }

    pub fn lowest_level(&self, level: vmm::PageLevel) -> Self {
        use vmm::PageLevel;

        match level {
            //Removes offset
            PageLevel::Level1 => Self(self.0 & (!0xfff)),
            PageLevel::Level2 => Self(self.0 & (!0x1fffff)),
            PageLevel::Level3 => Self(self.0 & (!0x3fffffff)),
            PageLevel::Level4 => Self(self.0 & (!0x7fffffffff)),
            PageLevel::Level5 => Self(self.0 & (!0xffffffffffff)),
            PageLevel::Root => Self(0)
        }
    }
}

pub fn virt_to_phys(virt: VirtualAddress) -> PhysicalAddress {
    PhysicalAddress(virt.0 - HHDM_OFFSET.load(Ordering::Relaxed))
}

pub fn phys_to_virt(phys: PhysicalAddress) -> VirtualAddress {
    VirtualAddress(phys.0 + HHDM_OFFSET.load(Ordering::Relaxed))
}

pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}

fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}