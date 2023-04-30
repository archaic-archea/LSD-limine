// SPDX-FileCopyrightText: © 2023 Archaic Archea <archaic.archea@gmail.com>
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::{sync::atomic::{Ordering, AtomicU64}, cell::SyncUnsafeCell};
use linked_list::LinkedListAllocator;

pub mod pmm;
pub mod vmm;
pub mod linked_list;

pub static HEAP: SyncUnsafeCell<[u8; 16384]> = SyncUnsafeCell::new([0; 16384]);

#[global_allocator]
pub static ALLOCATOR: Locked<LinkedListAllocator> = Locked::new(LinkedListAllocator::new());

pub static HHDM_OFFSET: AtomicU64 = AtomicU64::new(0);

pub fn init_tls() {
    use super::utils::linker;

    unsafe {
        let tdata_start = linker::__tdata_start.as_usize();
        let tdata_end = linker::__tdata_end.as_usize();

        let tdata_size = tdata_end - tdata_start;

        let mut frames = tdata_size / 4096;
        
        if (tdata_size & 0xfff) != 0 {
            frames += 1;
        }

        let tls_base = pmm::REGION_LIST.lock().claim_continuous(frames).unwrap();

        for offset in 0..tdata_size {
            let read_addr = linker::__tdata_start.as_ptr().byte_add(offset);
            let write_addr = tls_base.byte_add(offset);

            let read = *read_addr;
            write_addr.write(read);
        }

        core::arch::asm!(
            "mv tp, {tls}",
            tls = in(reg) tls_base
        );
    }
}

bitfield::bitfield! {
    #[derive(Copy, Clone)]
    #[repr(transparent)]
    pub struct PhysicalAddress(u64);
    impl Debug;
    u64;
    get_page_offset, set_page_offset: 11, 0;
    get_ppn0, set_ppn0: 20, 12;
    get_ppn1, set_ppn1: 29, 21;
    get_ppn2, set_ppn2: 38, 30;
    get_ppn3, set_ppn3: 47, 39;
    get_ppn4, set_ppn4: 55, 48;

    get_ppn, set_ppn: 55, 12;
}

impl PhysicalAddress {
    pub const fn null() -> Self {
        Self(0)
    }

    pub fn as_ptr(&self) -> *mut u8 {
        (self.0 + HHDM_OFFSET.load(Ordering::Relaxed)) as *mut u8
    }

    pub fn from_ptr(ptr: *mut u8) -> Self {
        Self(ptr as u64 - HHDM_OFFSET.load(Ordering::Relaxed))
    }

    pub fn add(&self, rhs: u64) -> Self {
        Self(self.0 + rhs)
    }

    pub fn new(val: u64) -> Self {
        Self(val)
    }
}

bitfield::bitfield! {
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
    #[repr(transparent)]
    pub struct VirtualAddress(u64);
    impl Debug;
    u64;
    get_page_offset, set_page_offset: 11, 0;
    get_vpn0, set_vpn0: 20, 12;
    get_vpn1, set_vpn1: 29, 21;
    get_vpn2, set_vpn2: 38, 30;
    get_vpn3, set_vpn3: 47, 39;
    get_vpn4, set_vpn4: 56, 48;
}

impl VirtualAddress {
    pub const fn null() -> Self {
        Self(0)
    }

    pub fn from_ptr(ptr: *mut u8) -> Self {
        Self(ptr as u64)
    }

    pub fn as_ptr(&self) -> *mut u8 {
        self.0 as *mut u8
    }

    pub fn add(&self, rhs: u64) -> Self {
        Self(self.0 + rhs)
    }

    pub fn new(val: u64) -> Self {
        Self(val)
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
            PageLevel::PageOffset => *self
        }
    }

    pub fn index(&self, index: vmm::PageLevel) -> u64 {
        use vmm::PageLevel;

        match index {
            PageLevel::Level1 => self.get_vpn0(),
            PageLevel::Level2 => self.get_vpn1(),
            PageLevel::Level3 => self.get_vpn2(),
            PageLevel::Level4 => self.get_vpn3(),
            PageLevel::Level5 => self.get_vpn4(),
            PageLevel::PageOffset => self.get_page_offset(),
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