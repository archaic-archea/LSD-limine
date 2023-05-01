// SPDX-FileCopyrightText: © 2023 Archaic Archea <archaic.archea@gmail.com>
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use spin::Mutex;

use crate::println;

pub static REGION_LIST: Mutex<FreeList> = Mutex::new(FreeList::null());

/// # Safety
/// Only call once
pub unsafe fn init(map: &limine::MemoryMap) {
    let mut uninit = true;

    for entry in map.entries() {
        if entry.kind() == limine::MemoryKind::Usable {
            let base = entry.base + super::HHDM_OFFSET.load(core::sync::atomic::Ordering::Relaxed) as usize;

            for offset in (0..entry.size).step_by(4096) {
                let new = base + offset;

                if uninit {
                    REGION_LIST.try_lock().unwrap().init(new as *mut u8);
                    uninit = false;
                } else {
                    REGION_LIST.try_lock().unwrap().push(new as *mut u8);
                }
            }
        }
    }
    println!("Initialized pmm");
}

pub struct FreeList {
    head: *mut FreeListEntry,
    tail: *mut FreeListEntry,
    len: usize,
}

impl FreeList {
    const fn null() -> Self {
        FreeList { 
            head: 0 as *mut FreeListEntry, 
            tail: 0 as *mut FreeListEntry, 
            len: 0 
        }
    }

    /// # Safety
    /// Only call once when initializing the free list
    pub unsafe fn init(&mut self, new: *mut u8) {
        let new_entry = new as *mut FreeListEntry;
        new_entry.write_volatile(FreeListEntry::null());

        self.head = new_entry;
        self.tail = new_entry;

        self.len = 1;
    }

    /// Appends a new entry to the end of the list
    /// # Safety
    /// Only call on memory that is being unused, and wont be used later(unless calling `claim`)
    pub unsafe fn push(&mut self, new: *mut u8) {
        let new = new as *mut FreeListEntry;

        // Check that the new entry isn't in page 0, and is aligned
        if (new as usize) < 0x1000 {
            panic!("Attempt attempt to push entry in page 0");
        } else if !new.is_aligned() {
            panic!("Bad alignment for entry");
        }

        // Setup new entry
        (*new).next = core::ptr::null_mut();
        (*new).prev = self.tail;

        // Link the new entry and set new tail
        (*self.tail).next = new;
        self.tail = new;

        // Increment length
        self.len += 1;
    }

    /// Appends a new entry to the start of the list
    /// # Safety
    /// Only call on memory that is being unused, and wont be used later(unless calling `claim`)
    pub unsafe fn pull(&mut self, new: *mut u8) {
        let new = new as *mut FreeListEntry;

        // Check that the new entry isn't in page 0, and is aligned
        if (new as usize) < 0x1000 {
            panic!("Attempt attempt to push entry in page 0");
        } else if !new.is_aligned() {
            panic!("Bad alignment for entry");
        }

        // Setup new entry
        (*new).prev = core::ptr::null_mut();
        (*new).next = self.head;

        // Link the new entry and set new head
        (*self.head).prev = new;
        self.head = new;

        // Increment length
        self.len += 1;
    }

    pub fn claim(&mut self) -> *mut u8 {
        // Temporarily store original head and next entry pointer
        let og_head = self.head;
        let next_entry = unsafe {(*og_head).next};

        // Set next entry's `prev` field to null
        unsafe {(*next_entry).prev = core::ptr::null_mut()};
        
        // Reassign head to the free entry
        self.head = next_entry;
        
        og_head as *mut u8
    }

    pub fn claim_continuous(&mut self, frames: usize) -> Result<*mut u8, alloc::string::String> {
        if frames == 1 {
            return Ok(self.claim());
        }

        // Store the head as a current entry, as well as the base entry of this contigous section
        let mut base_entry = self.head;
        let mut current_entry = self.head;
        let mut contiguous_frames = 1;

        for _ in 0..self.len {
            // If we found the needed amount of frames handle returning the base entry
            // Otherwise handle getting the next entry
            if contiguous_frames == frames {
                unsafe {
                    let previous = (*base_entry).prev;
                    let next = (*current_entry).next;

                    // Reassign the head if needed, otherwise stitch the previous, and next entries together
                    if previous.is_null() {
                        self.head = (*current_entry).next;
                        (*next).prev = core::ptr::null_mut();
                    } else {
                        (*previous).next = next;
                        (*next).prev = previous;
                    }

                    return Ok(base_entry as *mut u8);
                }
            } else {
                unsafe {
                    // If the current entry is contigous with the next entry, increment `contigous_frames` and set the current entry to the next
                    // Otherwise reset `contigous_frames`, set the current entry to the next entry, set the base entry to the current entry
                    if (current_entry as usize + 0x1000) == (*current_entry).next as usize {
                        current_entry = (*current_entry).next;
                        contiguous_frames += 1;
                    } else {
                        current_entry = (*current_entry).next;
                        base_entry = current_entry;
                        contiguous_frames = 1;
                    }
                }
            }
        }

        // Return none if we couldnt find a contigous piece of memory large enough
        Err(alloc::format!("Couldnt find contiguous frames out of {} frames", self.len))
    }
}

unsafe impl Send for FreeList {}
unsafe impl Sync for FreeList {}

#[derive(PartialEq, Debug)]
#[repr(align(4096))]
struct FreeListEntry {
    prev: *mut FreeListEntry,
    next: *mut FreeListEntry,
}

impl FreeListEntry {
    pub fn null() -> Self {
        Self { 
            prev: core::ptr::null_mut::<FreeListEntry>(), 
            next: core::ptr::null_mut::<FreeListEntry>() 
        }
    }
}