// SPDX-FileCopyrightText: © 2023 Archaic Archea <archaic.archea@gmail.com>
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::sync::atomic::{Ordering, AtomicU8};

use spin::MutexGuard;

use crate::println;

use super::{VirtualAddress, PhysicalAddress, pmm};

pub static LEVELS: AtomicU8 = AtomicU8::new(0);

pub struct Vmm <'a, 'b>(vmem::Vmem<'a, 'b>);

impl<'a, 'b> Vmm <'a, 'b> {
    pub const fn new(name: &'static str) -> Self {
        Self(vmem::Vmem::new(alloc::borrow::Cow::Borrowed(name), 1, None))
    }

    pub fn alloc(&self, size: usize, strategy: vmem::AllocStrategy) -> Result<usize, vmem::Error> {
        let section = self.0.alloc(size, strategy);

        if !section.is_err() {
            let section_data = section.as_ref().unwrap();

            let mut frames = size / 4096;

            if (size % 4096) != 0 {
                frames += 1;
            }

            for offset in (0..size).step_by(4096) {
                let claim = pmm::REGION_LIST.lock().claim_frames(frames).unwrap();
                let claim_phys = PhysicalAddress((claim as u64) - super::HHDM_OFFSET.load(Ordering::Relaxed));

                let level = PageLevel::from_usize(
                    LEVELS.load(Ordering::Relaxed)as usize
                );

                let virt = VirtualAddress((*section_data + offset) as u64);

                map(current_table(), virt, claim_phys, level, PageLevel::Level1, &mut pmm::REGION_LIST.lock());

                flush_tlb(Some(virt), None);
            }
        }

        return section;
    }

    pub unsafe fn free(&self, base: usize, size: usize) {
        self.0.free(base, size);

        for offset in (0..size).step_by(4096) {
            let virt = VirtualAddress((base + offset) as u64);

            let level = PageLevel::from_usize(
                LEVELS.load(Ordering::Relaxed)as usize
            );

            let phys = unmap(current_table(), virt, level).0 + super::HHDM_OFFSET.load(Ordering::Relaxed);

            super::pmm::REGION_LIST.lock().shove(phys as *mut u8);

            flush_tlb(Some(virt), None);
            return;
        }
    }

    pub fn add(&self, base: usize, size: usize) -> Result<(), vmem::Error> {
        self.0.add(base, size)
    }
}

pub fn flush_tlb(vaddr: Option<VirtualAddress>, asid: Option<u16>) {
    unsafe {
        match (vaddr, asid) {
            (Some(vaddr), Some(asid)) => {
                let vaddr = vaddr.0;
                core::arch::asm!("sfence.vma {}, {}", in(reg) vaddr, in(reg) asid);
            }
            (Some(vaddr), None) => {
                let vaddr = vaddr.0;
                core::arch::asm!("sfence.vma {}, zero", in(reg) vaddr);
            }
            (None, Some(asid)) => core::arch::asm!("sfence.vma zero, {}", in(reg) asid),
            (None, None) => core::arch::asm!("sfence.vma zero, zero"),
        }
    }
}

pub fn init() {
    let fdt_ptr = super::super::FDT_PTR.lock().clone() as *const u8;
    let fdt = unsafe {fdt::Fdt::from_ptr(fdt_ptr).expect("Invalid FDT ptr")};
    let node = fdt.find_node("/cpus/cpu@0").unwrap();
    let mmu_type = node.property("mmu-type").unwrap().as_str().unwrap();

    let mmu_type = match mmu_type {
        "riscv,sv39" => {
            LEVELS.store(3, Ordering::Relaxed);
            PageType::Sv39
        },
        "riscv,sv48" => {
            LEVELS.store(4, Ordering::Relaxed);
            PageType::Sv48
        },
        "riscv,sv57" => {
            LEVELS.store(5, Ordering::Relaxed);
            PageType::Sv57
        },
        _ => unreachable!()
    };

    println!("MMU type: {:?}", mmu_type);

    let node = fdt.find_node("/memory@80000000").unwrap();
    let mut memory = node.reg().unwrap();
    let memory_size = memory.next().unwrap().size.unwrap();

    println!("Memory size: {:?}MiB", memory_size / 1048576);

    let root_table_claim = pmm::REGION_LIST.lock().claim() as *mut PageTable;

    clone_table_range(current_table(), root_table_claim, 256..512);

    let mut reg_list_lock = super::pmm::REGION_LIST.lock();

    println!("Mapping IO");
    for i in (0..0x8000_0000_u64).step_by(PageSize::Large as usize) {
        let virt = VirtualAddress::new(0xffffffff80000000).add(i as u64);
        //let virt = VirtualAddress::new(0x00).add(i as u64);
        let phys = PhysicalAddress::new(0x00).add(i as u64);

        let level = PageLevel::from_usize(
            LEVELS.load(Ordering::Relaxed)as usize
        );
        
        println!("{:?} 0x{:x} -> 0x{:x}", level, virt.0, phys.0);

        map(root_table_claim, virt, phys, level, PageLevel::Level3, &mut reg_list_lock);
    }
    println!("Mapped IO");

    super::HHDM_OFFSET.store(mmu_type.hhdm_offset() as u64, Ordering::Relaxed);
    let mut new_satp = Satp(0);
    new_satp.set_ppn(
        (
            (
                root_table_claim as u64
            ) - super::HHDM_OFFSET.load(Ordering::Relaxed)
        ) >> 12
    );

    new_satp.set_mode(PageType::Sv48 as u64);

    unsafe {
        new_satp.set();
    }
    crate::uart::UART.lock().0 = 0xffffffff90000000 as *mut crate::uart::Uart16550;
}

pub fn clone_table_range(src: *const PageTable, dest: *mut PageTable, range: core::ops::Range<usize>) {
    unsafe {
        for index in range {
            let entry = &(*src).0[index];
            let new_entry = &mut (*dest).0[index];

            if entry.is_branch() {
                *new_entry = *entry;

                let new_claim = pmm::REGION_LIST.lock().claim() as u64;
                let phys_new_claim = new_claim - super::HHDM_OFFSET.load(Ordering::Relaxed);

                new_entry.set_ppn(phys_new_claim >> 12);

                clone_table_range(entry.table(), new_entry.table(), 0..512);
            } else if entry.is_leaf() {
                *new_entry = *entry;
            } else {
                *new_entry = PageEntry(0);
            }
        }
    }
}

pub fn current_table() -> *mut PageTable {
    let satp = Satp::new();

    let ppn = satp.get_ppn();
    let phys = ppn << 12;

    let virt = phys + super::HHDM_OFFSET.load(Ordering::Relaxed);

    return virt as *mut PageTable;
}

pub fn unmap(
    table: *mut PageTable,
    virt: VirtualAddress,
    level: PageLevel
) -> PhysicalAddress {
    let mut table = table;
    let mut level = level;

    loop {
        unsafe {
            let table_index = virt.index(level.as_usize());

            let entry = &mut (*table).0[table_index as usize];

            if entry.is_leaf() {
                println!("Found entry to unmap");
                let return_addr = entry.get_ppn() << 12;
                entry.0 = 0;

                return PhysicalAddress(return_addr);
            } else if entry.is_branch() {
                let next_table_phys = entry.get_ppn() << 12;
                let next_table = next_table_phys + super::HHDM_OFFSET.load(Ordering::Relaxed);

                table = next_table as *mut PageTable;
            } else {
                panic!("No entry found for virt 0x{:x}", virt.0);
            }

            if level == PageLevel::Level1 {
                panic!("Reached level 1");
            }

            level = PageLevel::from_usize(level.as_usize() - 1);
        }
    }
}

pub fn map(
    table: *mut PageTable,
    virt: VirtualAddress, 
    phys: PhysicalAddress, 
    level: PageLevel, 
    target_level: PageLevel,
    pmm_lock: &mut MutexGuard<super::pmm::FreeList>,
) {
    let mut table = table;
    let mut level = level;

    loop {
        unsafe {
            let table_index = virt.index(level.as_usize());
            
            let entry = &mut (*table).0[table_index as usize];

            if level == target_level {
                //println!("Making leaf at index {} of table {:?}", table_index, table);
                entry.set_ppn(phys.get_ppn());
                entry.set_valid(true);
                entry.set_read(true);
                entry.set_write(true);
                entry.set_exec(true);
                return;
            }

            if entry.is_branch() {
                //println!("Found branch at index {} of table {:?}", table_index, table);
                table = ((entry.get_ppn() << 12) + super::HHDM_OFFSET.load(core::sync::atomic::Ordering::Relaxed)) as *mut PageTable;
            } else if entry.is_leaf() {
                panic!("Didnt expect leaf at index {} of table {:?}", table_index, table);
            } else if !entry.get_valid() {
                //println!("Making branch at index {} of table {:?}", table_index, table);
                let new_table = pmm_lock.claim() as *mut PageTable;
                let new_table_phys = (new_table as u64) - super::HHDM_OFFSET.load(core::sync::atomic::Ordering::Relaxed);

                entry.set_ppn(new_table_phys >> 12);
                entry.set_valid(true);
                table = new_table;
            }

            level = PageLevel::from_usize(level.as_usize() - 1);
        }
    }
}

#[repr(u64)]
pub enum PageSize {
    Small = 0x1000,
    Medium = 0x20_0000,
    Large = 0x4000_0000,
    Huge = 0x80_0000_0000,
}

#[derive(Debug)]
pub enum PageType {
    Bare = 0,
    Sv39 = 8,
    Sv48 = 9,
    Sv57 = 10
}

impl PageType {
    pub fn hhdm_offset(&self) -> usize {
        match self {
            Self::Bare => 0x00,
            Self::Sv39 => 0xffffffc000000000,
            Self::Sv48 => 0xffff800000000000,
            Self::Sv57 => 0xff00000000000000
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub enum PageLevel {
    Level5,
    Level4,
    Level3,
    Level2,
    Level1,
    PageOffset
}

impl PageLevel {
    pub fn from_usize(val: usize) -> Self {
        match val {
            1 => Self::Level1,
            2 => Self::Level2,
            3 => Self::Level3,
            4 => Self::Level4,
            5 => Self::Level5,
            _ => Self::PageOffset,
        }
    }

    pub fn as_usize(&self) -> usize {
        match self {
            Self::Level1 => 1,
            Self::Level2 => 2,
            Self::Level3 => 3,
            Self::Level4 => 4,
            Self::Level5 => 5,
            _ => 0
        }
    }
}

use core::ops;

impl ops::Sub<usize> for PageLevel {
    type Output = PageLevel;

    fn sub(self, rhs: usize) -> Self::Output {
        Self::from_usize(self.as_usize() - rhs)
    }
}

impl ops::Add<usize> for PageLevel {
    type Output = PageLevel;

    fn add(self, rhs: usize) -> Self::Output {
        let load = LEVELS.load(Ordering::Relaxed);
        
        println!("load res: {self:?}, {:#x}", load);
        if self.as_usize() + 1 < load as usize {
            println!("ret {:?}", Self::from_usize(self.as_usize() + rhs));
            Self::from_usize(self.as_usize() + rhs)
        } else {
            println!("ret page offset");
            Self::PageOffset
        }
    }
}

pub struct PageTable([PageEntry; 512]);

bitfield::bitfield!{
    #[derive(Copy, Clone)]
    struct PageEntry(u64);
    u64;
    get_valid, set_valid: 0;
    get_read, set_read: 1;
    get_write, set_write: 2;
    get_exec, set_exec: 3;
    get_user, set_user: 4;
    get_global, set_global: 5;
    get_accessed, set_accessed: 6;
    get_dirty, set_dirty: 7;
    get_rsw, set_rsw: 9, 8;
    get_ppn, set_ppn: 53, 10;
    get_reserved, set_reserved: 60, 54;
    get_pbmt, set_pbmt: 62, 61;
    get_n, set_n: 63;
}

impl PageEntry {
    pub fn is_branch(&self) -> bool {
        return self.get_valid() && !self.get_read() && !self.get_write() && !self.get_exec();
    }

    pub fn is_leaf(&self) -> bool {
        return self.get_valid() && (self.get_read() || self.get_write() || self.get_exec());
    }

    pub fn table(&self) -> *mut PageTable {
        let table_phys = self.get_ppn() << 12;
        let table = table_phys + super::HHDM_OFFSET.load(Ordering::Relaxed);

        return table as *mut PageTable;
    }
}

bitfield::bitfield!{
    struct Satp(u64);
    u64;
    get_ppn, set_ppn: 43, 0;
    get_asid, set_asid: 59, 44;
    get_mode, set_mode: 63, 60;
}

impl Satp {
    pub unsafe fn set(&self) {
        core::arch::asm!("csrw satp, {new}", new = in(reg) self.0);
    }

    pub fn new() -> Self {
        let new_satp: u64;

        unsafe {
            core::arch::asm!("csrr {new}, satp", new = out(reg) new_satp);
        }

        let new_satp = unsafe {core::mem::transmute(new_satp)};

        return new_satp;
    }
}
