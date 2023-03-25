// SPDX-FileCopyrightText: © 2023 Archaic Archea <archaic.archea@gmail.com>
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::println;

pub static REGION_LIST: Mutex<RegionList> = Mutex::new(RegionList::null());

pub fn init() {
    let memory_map = crate::MEM_MAP.get_response().get().expect("Memory map request not fulfilled");
    let regions = memory_map.memmap();

    let mut is_uninit = true;

    for region in regions.iter() {
        if region.typ == LimineMemoryMapEntryType::Usable {
            if (region.base != 0) && (region.len != 0) {
                if is_uninit {
                    is_uninit = false;
                    REGION_LIST.lock().init(region.base as *mut u8, (region.len / 4096) as usize);
                } else {
                    REGION_LIST.lock().push(region.base as *mut u8, (region.len / 4096) as usize);
                }
            }
        }
    }
}

//Region entries must be placed at the Beggining of an entry
#[derive(Clone, Copy)]
pub struct RegionEntry {
    /// Length in small pages
    len: usize,
    previous: PhysicalAddress,
    next: PhysicalAddress
}

pub struct RegionList {
    head: PhysicalAddress,
    tail: PhysicalAddress,
    len: usize
}

unsafe impl Send for RegionList {}
unsafe impl Sync for RegionList {}

#[derive(Clone)]
pub struct RegionIndex(Option<usize>);

impl RegionList {
    pub const fn null() -> Self {
        Self { 
            head: PhysicalAddress::null(), 
            tail: PhysicalAddress::null(), 
            len: 0
        }
    }

    pub fn init(&mut self, start: *mut u8, length: usize) {
        let entry_ptr = start as *mut RegionEntry;

        unsafe {
            (*entry_ptr).next = PhysicalAddress::null();
            (*entry_ptr).previous = PhysicalAddress::null(); 
            (*entry_ptr).len = length - 1; 
        }

        *self = Self {
            head: PhysicalAddress::from_ptr(entry_ptr.cast()),
            tail: PhysicalAddress::from_ptr(entry_ptr.cast()),
            len: 1
        };
    }

    pub fn new(start: *mut u8, length: usize) -> Self {
        let entry_ptr = start as *mut RegionEntry;

        unsafe {
            (*entry_ptr).next = PhysicalAddress::null();
            (*entry_ptr).previous = PhysicalAddress::null(); 
            (*entry_ptr).len = length - 1; 
        }

        Self {
            head: PhysicalAddress::from_ptr(entry_ptr.cast()),
            tail: PhysicalAddress::from_ptr(entry_ptr.cast()),
            len: 1
        }
    }

    /// Add an entry to the end
    pub fn push(&mut self, base: *mut u8, length: usize) {
        unsafe {
            let new_entry = base as *mut RegionEntry;

            let old_entry = self.tail.as_ptr() as *mut RegionEntry;
            (*old_entry).next = virt_to_phys(VirtualAddress::from_ptr(new_entry.cast()));

            (*new_entry).len = length - 1;
            (*new_entry).previous = PhysicalAddress::from_ptr(old_entry.cast());
            (*new_entry).next = self.head;

            (*((*new_entry).next.as_ptr() as *mut RegionEntry)).previous = PhysicalAddress::from_ptr(new_entry.cast());

            self.tail = PhysicalAddress::from_ptr(new_entry.cast());

            self.len += 1;
        }
    }

    /// Add an entry to the beginning
    pub fn shove(&mut self, base: *mut u8, length: usize) {
        unsafe {
            let new_entry = base as *mut RegionEntry;

            let old_entry = self.head.as_ptr() as *mut RegionEntry;
            (*old_entry).next = virt_to_phys(VirtualAddress::from_ptr(new_entry.cast()));

            (*new_entry).len = length - 1;
            (*new_entry).next = PhysicalAddress::from_ptr(old_entry.cast());
            (*new_entry).previous = self.tail;

            (*((*new_entry).next.as_ptr() as *mut RegionEntry)).previous = PhysicalAddress::from_ptr(new_entry.cast());

            self.head = PhysicalAddress::from_ptr(new_entry.cast());

            self.len += 1;
        }
    }

    pub fn insert(&mut self, index: usize, base: *mut u8, length: usize) {
        unsafe {
            let new_entry = base as *mut RegionEntry;

            let old_entry = core::ptr::addr_of_mut!(self[index - 1]);
            let old_next_entry = core::ptr::addr_of_mut!(self[index]);
            (*old_entry).next = PhysicalAddress::from_ptr(new_entry.cast());

            (*old_next_entry).previous = PhysicalAddress::from_ptr(new_entry.cast());

            (*new_entry).len = length - 1;
            (*new_entry).next = PhysicalAddress::from_ptr(old_next_entry.cast());
            (*new_entry).previous = PhysicalAddress::from_ptr(old_entry.cast());

            self.len += 1;
        }
    }

    /// Claims a small page and returns a pointer to the first element
    pub fn claim(&mut self, index: usize) -> (MaybeUninit<*mut u8>, RegionIndex) {
        unsafe {
            let entry = core::ptr::addr_of_mut!(self[index]);
            let prev_entry = (*entry).previous.as_ptr() as *mut RegionEntry;
            let next_entry = (*entry).next.as_ptr() as *mut RegionEntry;

            if (*entry).len == 0 {
                return (MaybeUninit::new(core::ptr::null_mut()), RegionIndex(None));
            } else {
                (*entry).len -= 1;
                *entry.byte_add(4096) = *entry;

                (*prev_entry).next = PhysicalAddress::from_ptr(entry.byte_add(4096).cast());
                (*next_entry).previous = PhysicalAddress::from_ptr(entry.byte_add(4096).cast());

                return (MaybeUninit::new(entry.cast()), RegionIndex(Some(index)));
            }
        }
    }

    pub fn return_claim(&mut self, index: RegionIndex) {
        match index.0 {
            Some(index) => {
                unsafe {
                    let entry = core::ptr::addr_of_mut!(self[index]);
                    let prev_entry = (*entry).previous.as_ptr() as *mut RegionEntry;
                    let next_entry = (*entry).next.as_ptr() as *mut RegionEntry;

                    (*entry).len += 1;
                    *entry.byte_sub(4096) = *entry;

                    (*prev_entry).next = PhysicalAddress::from_ptr(entry.byte_sub(4096).cast());
                    (*next_entry).previous = PhysicalAddress::from_ptr(entry.byte_sub(4096).cast());
                }
            },
            None => {}
        }
    }

    pub fn claim_page(&mut self) -> (MaybeUninit<*mut u8>, RegionIndex) {
        println!("claim page called");

        for index in 0..self.len {
            println!("indexing {index}");

            use core::ops::Index;
            let indexed = self.index(index);
            println!("claim indexed {index}");
            
            if (*indexed).len != 0 {
                println!("valid entry found");
                let claim = self.claim(index);
                println!("claimed");
                return claim;
            } else {
                println!("Null entry found, skipping");
            }
        }

        println!("No valid entry found, returning null");
        (MaybeUninit::new(core::ptr::null_mut()), RegionIndex(None))
    }

    pub fn claim_zeroed(&mut self) -> (*mut u8, RegionIndex) {
        println!("Claim zeroed called");
        let claim = self.claim_page();

        println!("Claimed, zeroing");
        match claim.1.0 {
            Some(index) => {
                unsafe {
                    let base = claim.0.assume_init();

                    for i in 0..4096 {
                        *base.add(i) = 0;
                    }

                    (
                        base,
                        RegionIndex(Some(index))
                    )
                }
            },
            _ => {
                (core::ptr::null_mut(), RegionIndex(None))
            }
        }
    }
}

use core::{ops, mem::MaybeUninit};

use limine::LimineMemoryMapEntryType;
use spin::Mutex;

use super::{PhysicalAddress, virt_to_phys, phys_to_virt, VirtualAddress};

impl ops::Index<usize> for RegionList {
    type Output = RegionEntry;

    fn index(&self, index: usize) -> &Self::Output {
        println!("Finding entry");

        println!("Forward searching...");
        let mut region_ptr = self.head;
        for _ in 0..index {
            unsafe {
                region_ptr = (*(phys_to_virt(region_ptr).as_ptr() as *mut RegionEntry)).next;
            }
        }

        println!("Found entry");
        return unsafe {&(*(phys_to_virt(region_ptr).as_ptr() as *mut RegionEntry))};
    }
}

impl ops::IndexMut<usize> for RegionList {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        println!("Finding entry");

        println!("Forward searching...");
        let mut region_ptr = self.head;
        for _ in 0..index {
            unsafe {
                region_ptr = (*(phys_to_virt(region_ptr).as_ptr() as *mut RegionEntry)).next;
            }
        }

        println!("Found entry");
        return unsafe {&mut (*(phys_to_virt(region_ptr).as_ptr() as *mut RegionEntry))};
    }
}