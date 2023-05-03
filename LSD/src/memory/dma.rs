// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::PhysicalAddress;
use core::{ptr::{NonNull, Pointee}, mem::MaybeUninit};

pub struct DmaRegion<T: ?Sized> {
    phys: PhysicalAddress,
    virt: NonNull<T>,
}

impl<T: Sized> DmaRegion<[MaybeUninit<T>]> {
    pub fn new_many(n_elements: usize) -> Self {
        use super::vmm::PageFlags;

        let layout = vmem::Layout::new(core::mem::size_of::<T>() * n_elements);
        let layout = layout.align(core::mem::align_of::<T>());

        let alloc = crate::HIGHER_HALF.alloc_constrained(
            layout, 
            vmem::AllocStrategy::BestFit, 
            true, 
            PageFlags::READ | PageFlags::WRITE
        ).unwrap();

        Self { 
            phys: alloc.1.unwrap(), 
            virt: core::ptr::NonNull::new(
                core::ptr::slice_from_raw_parts_mut(
                    alloc.0 as *mut core::mem::MaybeUninit<T>, 
                    n_elements
                )
            ).unwrap(),
        }
    }

    pub unsafe fn zeroed_many(n_elements: usize) -> Self {
        use super::vmm::PageFlags;

        let layout = vmem::Layout::new(core::mem::size_of::<T>() * n_elements);
        let layout = layout.align(core::mem::align_of::<T>());

        let alloc = crate::HIGHER_HALF.alloc_constrained(
            layout, 
            vmem::AllocStrategy::BestFit, 
            true, 
            PageFlags::READ | PageFlags::WRITE
        ).unwrap();

        for i in 0..(core::mem::size_of::<T>() * n_elements) {
            let ptr = alloc.0 as *mut u8;

            unsafe {
                *ptr.add(i) = 0;
            }
        }

        Self { 
            phys: alloc.1.unwrap(), 
            virt: core::ptr::NonNull::new(
                core::ptr::slice_from_raw_parts_mut(
                    alloc.0 as *mut core::mem::MaybeUninit<T>, 
                    n_elements
                )
            ).unwrap(),
        }
    }

    pub unsafe fn assume_init(self) -> DmaRegion<[T]> {
        let phys = self.phys;
        let virt = self.virt;
        core::mem::forget(self);

        DmaRegion { phys, virt: NonNull::slice_from_raw_parts(virt.cast(), virt.len()) }
    }
}

impl<T: Sized> DmaRegion<[T]> {
    pub fn get(&mut self, index: usize) -> Option<DmaElement<'_, T>> {
        if index < self.virt.len() {
            Some(DmaElement {
                phys: PhysicalAddress::new(self.phys.0 + (core::mem::size_of::<T>() * index) as u64),
                virt: unsafe { NonNull::new_unchecked(self.virt.as_ptr().get_unchecked_mut(index)) },
                _region: self,
            })
        } else {
            None
        }
    }
}

impl<T: ?Sized> DmaRegion<T> {
    pub unsafe fn new_raw(metadata: <T as Pointee>::Metadata, zero: bool) -> Self {
        use super::vmm::PageFlags;
        let size = core::mem::size_of_val_raw::<T>(core::ptr::from_raw_parts(core::ptr::null(), metadata));

        let layout = vmem::Layout::new(size);

        let alloc = crate::HIGHER_HALF.alloc_constrained(
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
            phys: alloc.1.unwrap(), 
            virt: core::ptr::NonNull::new(
                core::ptr::from_raw_parts_mut(alloc.0 as *mut (), metadata)
            ).unwrap(),
        }
    }

    pub fn physical_address(&self) -> PhysicalAddress {
        self.phys
    }

    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.virt.as_ptr() }
    }
}

impl<T> DmaRegion<MaybeUninit<T>> {
    pub unsafe fn new() -> Self
    where
        T: Pointee<Metadata = ()>,
    {
        todo!("new function from vanadinite");
        //let (phys, virt) = alloc_dma_memory(core::mem::size_of::<T>(), DmaAllocationOptions::NONE)?;
        //Result::Ok(Self { phys, virt: NonNull::from_raw_parts(virt.cast(), ()) })
    }

    pub unsafe fn zeroed() -> Self
    where
        T: Pointee<Metadata = ()>,
    {
        todo!("zeroed function from vanadinite");
        //let (phys, virt) = alloc_dma_memory(core::mem::size_of::<T>(), DmaAllocationOptions::ZERO)?;
        //Result::Ok(Self { phys, virt: NonNull::from_raw_parts(virt.cast(), ()) })
    }

    pub unsafe fn assume_init(self) -> DmaRegion<T> {
        let phys = self.phys;
        let virt = self.virt;
        core::mem::forget(self);

        DmaRegion { phys, virt: virt.cast() }
    }
}

// TODO: figure out if this is sound lol
impl<T: ?Sized> core::ops::Deref for DmaRegion<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.virt.as_ptr() }
    }
}

impl<T: ?Sized> core::ops::DerefMut for DmaRegion<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.virt.as_ptr() }
    }
}

impl<T: ?Sized> core::ops::Drop for DmaRegion<T> {
    // TODO: dealloc memory
    fn drop(&mut self) {}
}

pub struct DmaElement<'a, T> {
    phys: PhysicalAddress,
    virt: NonNull<T>,
    _region: &'a DmaRegion<[T]>,
}

impl<'a, T> DmaElement<'a, T> {
    pub fn physical_address(&self) -> PhysicalAddress {
        self.phys
    }

    pub fn get(&self) -> NonNull<T> {
        self.virt
    }
}