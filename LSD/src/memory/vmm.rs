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

    pub fn alloc(&self, size: usize, strategy: vmem::AllocStrategy, contiguous: bool) -> Result<usize, vmem::Error> {
        let section = self.0.alloc(size, strategy);

        if section.is_ok() {
            let section_data = section.as_ref().unwrap();

            let mut frames = size / 4096;

            if (size % 4096) != 0 {
                frames += 1;
            }

            let mut claim: Option<*mut u8>;
            let mut claim_phys = PhysicalAddress(0);

            if contiguous {
                claim = pmm::REGION_LIST.lock().claim_continuous(frames);
                claim_phys = PhysicalAddress((claim.unwrap() as u64) - super::HHDM_OFFSET.load(Ordering::Relaxed));
            }

            for offset in (0..size).step_by(4096) {
                if !contiguous {
                    claim = Some(pmm::REGION_LIST.lock().claim());
                    claim_phys = PhysicalAddress((claim.unwrap() as u64) - super::HHDM_OFFSET.load(Ordering::Relaxed));
                } else {
                    claim_phys.0 += 4096;
                }
                println!("Mapping 0x{:x}", claim_phys.0);

                let level = PageLevel::from_usize(
                    LEVELS.load(Ordering::Relaxed)as usize
                );

                let virt = VirtualAddress((*section_data + offset) as u64);

                unsafe {
                    map(current_table().cast_mut(), virt, claim_phys, level, PageLevel::Level1, &mut pmm::REGION_LIST.lock());
                }

                flush_tlb(Some(virt), None);
            }
        }

        section
    }

    /// # Safety
    /// The segment must have previously been allocated by a call to alloc().
    pub unsafe fn free(&self, base: usize, size: usize) {
        self.0.free(base, size);

        for offset in (0..size).step_by(4096) {
            let virt = VirtualAddress((base + offset) as u64);

            let level = PageLevel::from_usize(
                LEVELS.load(Ordering::Relaxed)as usize
            );

            let phys = unmap(current_table().cast_mut(), virt, level, PageLevel::Level1).0 + super::HHDM_OFFSET.load(Ordering::Relaxed);

            super::pmm::REGION_LIST.lock().pull(phys as *mut u8);

            flush_tlb(Some(virt), None);
        }
    }

    pub fn add(&self, base: usize, size: usize) -> Result<(), vmem::Error> {
        self.0.add(base, size)
    }
}

pub fn new_with_upperhalf() -> *mut PageTable {
    let new_table = pmm::REGION_LIST.lock().claim() as *mut PageTable;

    unsafe {
        clone_table_range(current_table(), new_table, 256..512);
    }

    new_table
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

/// # Safety
/// Must only be called once on the boot strap processor
pub unsafe fn init() {
    let fdt_ptr = (*crate::FDT_PTR.lock()) as *const u8;
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
    for entry in (*root_table_claim).0.iter_mut() {
        entry.0 = 0;
    }

    unsafe {
        clone_table_range(&*current_table(), &mut *root_table_claim, 256..512);
    }

    let mut reg_list_lock = super::pmm::REGION_LIST.lock();

    println!("Mapping IO");
    for i in (0..0x8000_0000_u64).step_by(PageSize::Large as usize) {
        let virt = VirtualAddress::new(0xffffffff80000000).add(i);
        let phys = PhysicalAddress::new(0x00).add(i);

        let level = PageLevel::from_usize(
            LEVELS.load(Ordering::Relaxed)as usize
        );
        
        println!("{:?} 0x{:x} -> 0x{:x}", level, virt.0, phys.0);

        unsafe {
            map(root_table_claim, virt, phys, level, PageLevel::Level3, &mut reg_list_lock);
        }
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

    println!("Virtual memory initialized");
}

/// FIXME: Somehow mutates source table
/// # Safety
/// Only run on an unloaded table
pub unsafe fn clone_table_range(src: *const PageTable, dest: *mut PageTable, range: core::ops::Range<usize>) {
    for index in range {
        let src_entry = (src as *mut PageEntry).add(index);
        let dest_entry = (dest as *mut PageEntry).add(index);

        if (*src_entry).is_branch() {
            *dest_entry = *src_entry;

            let new_claim = pmm::REGION_LIST.lock().claim() as u64;
            let phys_new_claim = new_claim - super::HHDM_OFFSET.load(Ordering::Relaxed);
            
            (*dest_entry).set_ppn(phys_new_claim >> 12);
            
            clone_table_range((*src_entry).table(), (*dest_entry).table().cast_mut(), 0..512);
        } else if (*src_entry).is_leaf() {
            *dest_entry = *src_entry;
        } else {
            *dest_entry = PageEntry(0);
        }
    }
}

pub fn current_table() -> *const PageTable {
    let satp = Satp::new();

    let ppn = satp.get_ppn();
    let phys = ppn << 12;

    let virt = phys + super::HHDM_OFFSET.load(Ordering::Relaxed);

    virt as *mut PageTable
}

/// # Safety
/// Only safe from a kernel perspective when unmapping the lower half
pub unsafe fn unmap(
    table: *mut PageTable,
    virt: VirtualAddress,
    level: PageLevel,
    target_level: PageLevel,
) -> PhysicalAddress {
    let mut table = table;
    let mut level = level;

    loop {
        let table_index = virt.index(level);

        if level == target_level {
            let mut table_copy = table.read_volatile();
            let entry = &mut table_copy.0[table_index as usize];

            if !entry.is_leaf() {
                panic!("No leaf found while unmapping");
            }
            
            //println!("Leaf at index {} of table {:?}", table_index, table);
            let return_addr = entry.get_ppn() << 12;
            entry.0 = 0;

            //println!("Old table dump: \n{:?}", table.read_volatile());
            //println!("New table dump: \n{:?}", table_copy);
            table.write_volatile(table_copy);

            return PhysicalAddress(return_addr);
        } else {
            let entry = table.read_volatile().0[table_index as usize];

            if entry.is_leaf() {
                panic!("Unexpected entry");
            } else if entry.is_branch() {
                //println!("Table at index {} of table {:?}", table_index, table);
                let next_table_phys = entry.get_ppn() << 12;
                let next_table = next_table_phys + super::HHDM_OFFSET.load(Ordering::Relaxed);

                //let tmp = table;
                table = next_table as *mut PageTable;

                //println!("Entry accessed: 0x{:x}", (*tmp).0[table_index as usize].0);
            } else {
                panic!("No entry found for virt 0x{:x}\ntable {:?}\ndump: {:#?}\nentry 0x{:x}", virt.0, table, *table, (*table).0[table_index as usize].0);
            }

            level = PageLevel::from_usize(level.as_usize() - 1);
        }
    }
}

/// # Safety
/// Only safe from a kernel perspective when mapping the lower half
pub unsafe fn map(
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
        let table_index = virt.index(level);

        if level == target_level {
            let mut table_copy = table.read_volatile();
            let entry = &mut table_copy.0[table_index as usize];

            //println!("Made leaf at index {} of table {:?}", table_index, table);
            entry.0 = 0;

            entry.set_ppn(phys.get_ppn());
            entry.set_valid(true);
            entry.set_read(true);
            entry.set_write(true);
            entry.set_exec(true);

            table.write_volatile(table_copy);
            return;
        } else {
            let mut table_copy = table.read_volatile();
            let entry = table_copy.0[table_index as usize];

            if entry.is_branch() {
                //println!("Found table at index {} of table {:?}", table_index, table);
                table = ((entry.get_ppn() << 12) + super::HHDM_OFFSET.load(core::sync::atomic::Ordering::Relaxed)) as *mut PageTable;
            } else if entry.is_leaf() {
                panic!("Didnt expect leaf at index {} of table {:?}", table_index, table);
            } else if !entry.get_valid() {
                let entry = &mut table_copy.0[table_index as usize];

                //println!("Made table at index {} of table {:?}", table_index, table);
                let new_table = pmm_lock.claim() as *mut PageTable;
                for entry in (*new_table).0.iter_mut() {
                    entry.0 = 0;
                }

                let new_table_phys = (new_table as u64) - super::HHDM_OFFSET.load(core::sync::atomic::Ordering::Relaxed);

                entry.set_ppn(new_table_phys >> 12);
                entry.set_valid(true);

                table.write_volatile(table_copy);

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

#[repr(transparent)]
pub struct PageTable(pub [PageEntry; 512]);

impl core::fmt::Debug for PageTable {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for (index, entry) in self.0.iter().enumerate() {
            if entry.get_valid() {
                write!(f, "index {index}: 0x{:x}", entry.0)?;
                if index < 511 {
                    writeln!(f, ",")?;
                }
            }
        }

        Ok(())
    }
}

bitfield::bitfield!{
    #[derive(Copy, Clone)]
    #[repr(transparent)]
    pub struct PageEntry(u64);
    impl Debug;
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
        self.get_valid() && (
            (!self.get_read()) && 
            (!self.get_write()) && 
            (!self.get_exec()) &&
            (!self.get_user()) &&
            (!self.get_accessed()) &&
            (!self.get_dirty())
        )
    }

    pub fn is_leaf(&self) -> bool {
        self.get_valid() && (self.get_read() || self.get_write() || self.get_exec())
    }

    pub fn table(&self) -> *const PageTable {
        let table_phys = self.get_ppn() << 12;
        let table = table_phys + super::HHDM_OFFSET.load(Ordering::Relaxed);

        table as *const PageTable
    }
}

bitfield::bitfield!{
    #[repr(transparent)]
    pub struct Satp(u64);
    
    pub get_ppn, set_ppn: 43, 0;
    pub get_asid, set_asid: 59, 44;
    pub get_mode, set_mode: 63, 60;
}

impl Satp {
    /// # Safety
    /// Only safe after having copied the upper-half of memory from the current map
    pub unsafe fn set(&self) {
        core::arch::asm!("csrw satp, {new}", new = in(reg) self.0);
        flush_tlb(None, None);
    }

    pub fn new() -> Self {
        let new_satp: u64;

        unsafe {
            core::arch::asm!("csrr {new}, satp", new = out(reg) new_satp);
        }

        unsafe {core::mem::transmute(new_satp)}
    }
}

impl Default for Satp {
    fn default() -> Self {
        Self::new()
    }
}

pub fn virt_to_phys(virt: VirtualAddress) -> Result<PhysicalAddress, &'static str> {
    let mut table = current_table();

    let mut levels = PageLevel::from_usize(LEVELS.load(Ordering::Relaxed) as usize);

    unsafe {
        loop {
            let entry = &(*table).0[virt.index(levels) as usize];

            if entry.is_leaf() {
                let mut addr = PhysicalAddress(0);

                let mask: u64 = match levels {
                    PageLevel::Level1 => 0xfff,
                    PageLevel::Level2 => 0x1f_ffff,
                    PageLevel::Level3 => 0x3fff_ffff,
                    PageLevel::Level4 => 0x7f_ffff_ffff,
                    PageLevel::Level5 => 0xffff_ffff_ffff,
                    _ => 0
                };

                let inv_mask = !mask;

                addr.0 &= inv_mask;
                addr.0 |= virt.0 & mask;

                println!("Mask level {levels:?}");
                println!("mask: 0b{mask:064b}\nvirt: 0b{:064b}", virt.0);

                return Ok(addr);
            } else if entry.is_branch() {
                table = entry.table();
            } else {
                return Err("Table not found");
            }

            levels = PageLevel::from_usize(levels.as_usize() - 1);
        }
    }
}