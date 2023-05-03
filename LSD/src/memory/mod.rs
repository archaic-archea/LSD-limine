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
pub mod dma;

pub use dma::*;

pub static HEAP: SyncUnsafeCell<[u8; 16384]> = SyncUnsafeCell::new([0; 16384]);

#[global_allocator]
pub static ALLOCATOR: Locked<LinkedListAllocator> = Locked::new(LinkedListAllocator::new());

pub static HHDM_OFFSET: AtomicU64 = AtomicU64::new(0);

/// # Safety
/// Can only be called once per core
pub unsafe fn init_tls() {
    use super::utils::linker;

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

/// Based off Repnops code, licensed under MPL-2.0
/*pub struct DmaRegion<T: ?Sized> {
    size: usize,
    phys: PhysicalAddress,
    virt: core::ptr::NonNull<T>,
}

impl<T: ?Sized> DmaRegion<T> {
    pub fn phys(&self) -> PhysicalAddress {
        self.phys
    }

    pub fn virt(&self) -> core::ptr::NonNull<T> {
        self.virt
    }

    pub fn leak(self) -> &'static mut Self {
        let boxed = alloc::boxed::Box::new(self);

        alloc::boxed::Box::leak(boxed)
    }
}

impl<T: Sized> DmaRegion<[core::mem::MaybeUninit<T>]> {
    pub fn new_many(amount: usize) -> Self {
        use vmm::PageFlags;

        let layout = vmem::Layout::new(core::mem::size_of::<T>() * amount);
        let layout = layout.align(core::mem::align_of::<T>());

        let alloc = super::HIGHER_HALF.alloc_constrained(
            layout, 
            vmem::AllocStrategy::BestFit, 
            true, 
            PageFlags::READ | PageFlags::WRITE
        ).unwrap();

        Self { 
            size: core::mem::size_of::<T>() * amount,
            phys: alloc.1.unwrap(), 
            virt: core::ptr::NonNull::new(
                core::ptr::slice_from_raw_parts_mut(
                    alloc.0 as *mut core::mem::MaybeUninit<T>, 
                    amount
                )
            ).unwrap(),
        }
    }

    pub fn zeroed_many(amount: usize) -> Self {
        use vmm::PageFlags;

        let layout = vmem::Layout::new(core::mem::size_of::<T>() * amount);
        let layout = layout.align(core::mem::align_of::<T>());

        let alloc = super::HIGHER_HALF.alloc_constrained(
            layout, 
            vmem::AllocStrategy::BestFit, 
            true, 
            PageFlags::READ | PageFlags::WRITE
        ).unwrap();

        for i in 0..(core::mem::size_of::<T>() * amount) {
            let ptr = alloc.0 as *mut u8;

            unsafe {
                *ptr.add(i) = 0;
            }
        }

        Self { 
            size: core::mem::size_of::<T>() * amount,
            phys: alloc.1.unwrap(), 
            virt: core::ptr::NonNull::new(
                core::ptr::slice_from_raw_parts_mut(
                    alloc.0 as *mut core::mem::MaybeUninit<T>, 
                    amount
                )
            ).unwrap(),
        }
    }

    /// # Safety
    /// Only use if you immediately initialize it, or already initialized it somehow
    pub unsafe fn assume_init(self) -> DmaRegion<[T]> {
        let phys = self.phys;
        let virt = self.virt;
        let size = self.size;
        core::mem::forget(self);

        DmaRegion { size, phys, virt: core::ptr::NonNull::slice_from_raw_parts(virt.cast(), virt.len()) }
    }
}

impl<T: ?Sized> DmaRegion<T> {
    /// # Safety
    /// Uhhhhhhhhh
    pub unsafe fn new_raw(metadata: <T as core::ptr::Pointee>::Metadata, zero: bool) -> Self {
        use vmm::PageFlags;
        let size = core::mem::size_of_val_raw::<T>(core::ptr::from_raw_parts(core::ptr::null(), metadata));

        let layout = vmem::Layout::new(size);

        let alloc = super::HIGHER_HALF.alloc_constrained(
            layout, 
            vmem::AllocStrategy::BestFit, 
            true, 
            PageFlags::READ | PageFlags::WRITE
        ).unwrap();

        if zero {
            let ptr = alloc.0 as *mut u8;
            
            for i in 0..size {
                *ptr.add(i) = 0;
            }
        }

        Self { 
            size,
            phys: alloc.1.unwrap(), 
            virt: core::ptr::NonNull::new(
                core::ptr::from_raw_parts_mut(alloc.0 as *mut (), metadata)
            ).unwrap(),
        }
    }
}

impl<T: ?Sized> core::ops::Drop for DmaRegion<T> {
    fn drop(&mut self) {
        unsafe {
            panic!("Dropping value");
            super::HIGHER_HALF.free_constrained(self.virt.addr().get(), core::mem::size_of_val(&self.size));
        }
    }
}*/

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
    pub get_page_offset, set_page_offset: 11, 0;
    pub get_vpn0, set_vpn0: 20, 12;
    pub get_vpn1, set_vpn1: 29, 21;
    pub get_vpn2, set_vpn2: 38, 30;
    pub get_vpn3, set_vpn3: 47, 39;
    pub get_vpn4, set_vpn4: 56, 48;

    pub get_vpns, set_vpns: 56, 12;
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

    pub fn set_index(&mut self, index: vmm::PageLevel, val: u64) {
        use vmm::PageLevel;

        match index {
            PageLevel::Level1 => self.set_vpn0(val),
            PageLevel::Level2 => self.set_vpn1(val),
            PageLevel::Level3 => self.set_vpn2(val),
            PageLevel::Level4 => self.set_vpn3(val),
            PageLevel::Level5 => self.set_vpn4(val),
            PageLevel::PageOffset => self.set_page_offset(val),
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