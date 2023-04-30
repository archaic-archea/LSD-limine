// SPDX-FileCopyrightText: © 2023 Archaic Archea <archaic.archea@gmail.com>
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use spin::Mutex;

use crate::println;
use super::VolatileCell;

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
    println!("Initialized pmm");
}

pub struct FreeList {
    head: VolatileCell<*mut FreeListEntry>,
    tail: VolatileCell<*mut FreeListEntry>,
    len: VolatileCell<usize>,
}

impl FreeList {
    const fn null() -> Self {
        FreeList { head: VolatileCell::new(core::ptr::null_mut()), tail: VolatileCell::new(core::ptr::null_mut()), len: VolatileCell::new(0) }
    }

    pub fn init(&mut self, new: *mut u8) {
        unsafe {
            let new = &mut *(new as *mut FreeListEntry);
            new.next.write(core::ptr::null_mut());
            new.prev.write(core::ptr::null_mut());

            self.head.write(new as *mut FreeListEntry);
            self.tail.write(new as *mut FreeListEntry);
            self.len.write(1);
        }
    }

    pub fn push(&mut self, new: *mut u8) {
        if (new as usize) < 0x1000 {
            panic!("Attempt attempt to push entry in page 0");
        }
        if ((new as usize) & 0xfff) != 0 {
            panic!("Bad alignment for entry");
        }

        unsafe {
            let tail = &mut *self.tail.read();
            let new = &mut *(new as *mut FreeListEntry);
            new.next.write(core::ptr::null_mut());
            new.prev.write(core::ptr::null_mut());

            tail.next.write(new as *mut FreeListEntry);
            new.prev.write(self.tail.read());
            
            self.tail.write(new as *mut FreeListEntry);

            self.len.write(self.len.read() + 1);
        }
    }

    pub fn shove(&mut self, new: *mut u8) {
        if (new as usize) < 0x1000 {
            panic!("Attempt attempt to push entry in page 0");
        }
        if ((new as usize) & 0xfff) != 0 {
            panic!("Bad alignment for entry");
        }

        unsafe {
            let head = &mut *self.head.read();
            let new = &mut *(new as *mut FreeListEntry);
            new.next.write(core::ptr::null_mut());
            new.prev.write(core::ptr::null_mut());

            head.prev.write(new as *mut FreeListEntry);
            new.next.write(self.head.read());
            
            self.head.write(new as *mut FreeListEntry);

            self.len.write(self.len.read() + 1);
        }
    }

    pub fn claim(&mut self) -> *mut u8 {
        let ret = self.head.read() as *mut u8;

        unsafe {
            let head = &mut *self.head.read();
            let next = &mut *head.next.read();
            next.prev.write(core::ptr::null_mut());
            self.head.write((*self.head.read()).next.read());

            for i in 0..4096 {
                ret.add(i).write_volatile(0);
            }
        }

        self.len.write(self.len.read() - 1);

        ret
    }

    pub fn claim_frames(&mut self, frames: usize) -> Option<*mut u8> {
        let mut cur_frames_found: usize = 1;

        if frames == 1 {
            return Some(self.claim());
        }

        unsafe {
            let mut current_base = Some(self.head);
            let mut base = Some(self.head);

            while base != None {
                if cur_frames_found == frames {
                    let prev = (*current_base.unwrap().read()).prev.read();
                    let next = (*base.unwrap().read()).next.read();

                    if prev.is_null() {
                        if next.is_null() || (self.len.read() == 0) {
                            panic!("No memory left, len: 0x{:x}", self.len.read());
                        }

                        self.head.write(next);
                    } else {
                        prev.read_volatile().next.write(next);
                    }

                    if next.is_null() {
                        if prev.is_null() || (self.len.read() == 0) {
                            panic!("No memory left, len: 0x{:x}", self.len.read());
                        }

                        self.tail.write(prev);
                    } else {
                        (*next).prev.write(prev);
                    }

                    self.len.write(self.len.read() - frames);

                    return Some(base.unwrap().read() as *mut u8);
                }

                let buffer = base.unwrap().read().read_volatile().next();

                if buffer == None {
                    panic!("No memory found out of {} frames hit at entry {:?}", self.len.read(), base.unwrap().read().read_volatile());
                }

                if (base.unwrap().read() as usize + 0x1000) == (buffer.unwrap().read() as usize) {
                    cur_frames_found += 1;
                } else {
                    cur_frames_found = 1;
                    current_base = buffer;
                }

                base = buffer;
            }
        }

        panic!("No memory found out of {} frames", self.len.read());
    }
}

unsafe impl Send for FreeList {}
unsafe impl Sync for FreeList {}

#[derive(PartialEq, Debug)]
struct FreeListEntry {
    prev: VolatileCell<*mut FreeListEntry>,
    next: VolatileCell<*mut FreeListEntry>,
}

impl Iterator for FreeListEntry {
    type Item = VolatileCell<*mut FreeListEntry>;

    fn next(&mut self) -> Option<VolatileCell<*mut FreeListEntry>> {
        if self.next.read() != core::ptr::null_mut() {
            Some(self.next)
        } else {
            None
        }
    }
}