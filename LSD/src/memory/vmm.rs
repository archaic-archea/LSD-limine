// SPDX-FileCopyrightText: © 2023 Archaic Archea <archaic.archea@gmail.com>
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::sync::atomic::{Ordering, AtomicU8};

use alloc::collections::BTreeMap;
use spin::{Mutex, MutexGuard};

use crate::println;

use super::{VirtualAddress, PhysicalAddress, pmm::RegionIndex};

pub struct VirtRegion {
    _base: VirtualAddress,
    _phys_base: PhysicalAddress,
}

pub static MAPPED_REGIONS: Mutex<BTreeMap<usize, VirtRegion>> = Mutex::new(BTreeMap::new());
pub static REGION_INDEXES: Mutex<BTreeMap<(PageLevel, VirtualAddress), (RegionIndex, PhysicalAddress)>> = Mutex::new(BTreeMap::new());
pub static LEVELS: AtomicU8 = AtomicU8::new(0);

pub static FDT_PTR: limine::LimineDtbRequest = limine::LimineDtbRequest::new(0);
pub static KERN_DAT: limine::LimineKernelAddressRequest = limine::LimineKernelAddressRequest::new(0);

pub fn init() {
    let mut reg_list_lock = super::pmm::REGION_LIST.lock();
    let mut reg_idx_lock = REGION_INDEXES.lock();

    let kern_dat = KERN_DAT.get_response().get().unwrap();

    let fdt_ptr = FDT_PTR.get_response().get().unwrap().dtb_ptr.as_ptr().unwrap().cast_const();
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

    let root_table_claim = reg_list_lock.claim_zeroed();
    reg_idx_lock.insert((PageLevel::Root, VirtualAddress::null()), (root_table_claim.1, PhysicalAddress::from_ptr(root_table_claim.0)));

    let (kernel_size, kernel_start) = {
        unsafe {
            let start = crate::utils::linker::KERNEL_START.as_usize();
            let end = crate::utils::linker::KERNEL_END.as_usize();

            (end - start, start)
        }
    };

    println!("Mapping kernel");
    for i in (0..kernel_size + 4095).step_by(4096) {
        let virt = VirtualAddress::new(kernel_start).add(i);
        let phys = PhysicalAddress::new(kern_dat.physical_base as usize).add(i);
        let level = PageLevel::from_usize(
            (
                LEVELS.load(Ordering::Relaxed) - 1
            ) as usize
        );

        println!("{phys:x?} -> {virt:x?}");
        map(virt, phys, level, &mut reg_idx_lock, &mut reg_list_lock);
    }

    super::HHDM_OFFSET.store(mmu_type.hhdm_offset(), Ordering::Relaxed);
    todo!("store to satp");
}

pub fn map<'a>(
    virt: VirtualAddress, 
    phys: PhysicalAddress, 
    level: PageLevel, 
    idx_lock: &mut MutexGuard<'a, BTreeMap<(PageLevel, VirtualAddress), (RegionIndex, PhysicalAddress)>>,
    pmm_lock: &mut MutexGuard<super::pmm::RegionList>,
) {
    println!("Recur");
    let prev_table = idx_lock.get(
        &(
            level + 1,
            virt.lowest_level(level + 1)
        )
    ).expect("[Failure: Severe] Previous table does not exist") as *const (RegionIndex, PhysicalAddress);

    loop {
        println!("loop");
        let table = idx_lock.get(
            &(
                level,
                virt.lowest_level(level)
            )
        );

        match table {
            Some(val) => {
                //Check if we're on the last branch level
                if level == PageLevel::Level2 {
                    todo!("Create valid entry using physical address of table in 'val' and physically address provided by 'phys'");

                    return;
                }
                
                todo!("Take physical address, convert to page table pointer, deref, find next place, check for entry, recursion")
            },
            None => {
                println!("Table not found, allocating");

                let new_claim = pmm_lock.claim_zeroed();
                println!("Table allocated, inserting into tree");
                let new_claim = (new_claim.1, PhysicalAddress::from_ptr(new_claim.0));

                let _ = idx_lock.insert(
                    (
                        level,
                        virt.lowest_level(level),
                    ), 
                    new_claim.clone()
                );

                let mut entry = PageEntry::new(new_claim.1);
                entry.valid();

                unsafe {
                    let prev_ptr = (*prev_table).1.as_ptr() as *mut PageTable;
                    (*prev_ptr).0[virt.index(level.as_usize())] = entry;
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum PageType {
    Sv39,
    Sv48,
    Sv57
}

impl PageType {
    pub fn hhdm_offset(&self) -> usize {
        match self {
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
    Root
}

impl PageLevel {
    pub fn from_usize(val: usize) -> Self {
        match val {
            0 => Self::Level1,
            1 => Self::Level2,
            2 => Self::Level3,
            3 => Self::Level4,
            4 => Self::Level5,
            _ => Self::Root,
        }
    }

    pub fn as_usize(&self) -> usize {
        match self {
            Self::Level1 => 0,
            Self::Level2 => 1,
            Self::Level3 => 2,
            Self::Level4 => 3,
            _ => 4
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
            println!("ret Root");
            Self::Root
        }
    }
}

pub struct PageTable([PageEntry; 512]);

pub struct PageEntry(u64);

impl PageEntry {
    const V: u64 = 0b1 << 0;
    const R: u64 = 0b1 << 1;
    const W: u64 = 0b1 << 2;
    const X: u64 = 0b1 << 3;
    const U: u64 = 0b1 << 4;
    const G: u64 = 0b1 << 5;
    const A: u64 = 0b1 << 6;
    const D: u64 = 0b1 << 7;

    pub fn new(addr: PhysicalAddress) -> Self {
        let addr = addr.0 & (!0xfff);
        let addr = addr >> 2;

        Self(addr as u64)
    }

    pub fn valid(&mut self) {
        self.0 |= Self::V;
    }

    pub fn read(&mut self) {
        self.0 |= Self::R;
    }

    pub fn write(&mut self) {
        self.0 |= Self::W;
    }

    pub fn execute(&mut self) {
        self.0 |= Self::X;
    }

    pub fn user_accessible(&mut self) {
        self.0 |= Self::U;
    }

    pub fn global(&mut self) {
        self.0 |= Self::G;
    }

    pub fn accessed(&mut self) {
        self.0 |= Self::A;
    }

    pub fn dirty(&mut self) {
        self.0 |= Self::D
    }
}