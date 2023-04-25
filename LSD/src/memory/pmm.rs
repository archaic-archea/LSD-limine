// SPDX-FileCopyrightText: © 2023 Archaic Archea <archaic.archea@gmail.com>
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use spin::Mutex;

use crate::println;

pub static REGION_LIST: Mutex<FreeList> = Mutex::new(FreeList::null());

pub fn init(map: &limine::MemoryMap) {
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
}

pub struct FreeList {
    head: *mut FreeListEntry,
    tail: *mut FreeListEntry,
}

impl FreeList {
    const fn null() -> Self {
        FreeList { head: core::ptr::null_mut(), tail: core::ptr::null_mut() }
    }

    pub fn init(&mut self, new: *mut u8) {
        unsafe {
            self.head = new as *mut FreeListEntry;
            (*self.head).next = core::ptr::null_mut();
            (*self.head).prev = core::ptr::null_mut();
            self.tail = new as *mut FreeListEntry;
        }
    }

    pub fn push(&mut self, new: *mut u8) {
        unsafe {
            let tail = &mut *self.tail;

            tail.next = new as *mut FreeListEntry;
            (*tail.next).prev = self.tail;
            
            self.tail = tail.next;
            (*self.tail).next = core::ptr::null_mut();
        }
    }

    pub fn shove(&mut self, new: *mut u8) {
        unsafe {
            let head: &mut FreeListEntry = &mut *self.head;

            head.prev = new as *mut FreeListEntry;
            (*head.prev).next = self.head;
            
            self.head = head.prev;
            (*self.head).prev = core::ptr::null_mut();
        }
    }

    pub fn claim(&mut self) -> *mut u8 {
        let ret = self.head as *mut u8;

        unsafe {
            (*(*self.head).next).prev = core::ptr::null_mut();
            self.head = (*self.head).next;

            for i in 0..4096 {
                *ret.add(i) = 0;
            }
        }

        ret
    }

    pub fn claim_frames(&mut self, frames: usize) -> Option<*mut u8> {
        let mut cur_frames_found: usize = 1;

        unsafe {
            let mut current_base = Some(self.head);
            let mut base = Some(self.head);

            while base != None {
                let buffer = (*base.unwrap()).next();

                if buffer == None {
                    return None;
                }

                if (base.unwrap() as usize + 0x1000) == (buffer.unwrap() as usize) {
                    cur_frames_found += 1;
                } else {
                    cur_frames_found = 1;
                    current_base = buffer;
                }

                base = buffer;

                if cur_frames_found == frames {
                    return Some(current_base.unwrap() as *mut u8);
                }
            }
        }
        
        None
    }
}

unsafe impl Send for FreeList {}
unsafe impl Sync for FreeList {}

#[derive(PartialEq)]
struct FreeListEntry {
    prev: *mut FreeListEntry,
    next: *mut FreeListEntry,
}

impl Iterator for FreeListEntry {
    type Item = *mut FreeListEntry;

    fn next(&mut self) -> Option<*mut FreeListEntry> {
        if self.next != core::ptr::null_mut() {
            Some(self.next)
        } else {
            None
        }
    }
}