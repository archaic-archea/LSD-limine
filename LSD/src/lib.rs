// SPDX-FileCopyrightText: © 2023 Archaic Archea <archaic.archea@gmail.com>
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]
#![feature(
    pointer_byte_offsets,
    sync_unsafe_cell,
    const_mut_refs,
    extend_one,
    thread_local,
    naked_functions,
    asm_const,
    fn_align,
    stdsimd,
    core_intrinsics,
    pointer_is_aligned,
    layout_for_ptr,
    ptr_metadata,
    strict_provenance,
    error_in_core,
    slice_ptr_get,
    exclusive_range_pattern,
    int_roundings
)]

extern crate alloc;

pub mod memory;
pub mod uart;
pub mod libs;
pub mod utils;
pub mod traps;
pub mod timing;
pub mod drivers;
pub mod userspace;

pub mod arch;

use alloc::vec::Vec;
pub use libs::*;
use spin::Mutex;
use core::cell::UnsafeCell;
use core::sync::atomic::{self, AtomicPtr, AtomicBool};

static LOWER_HALF: memory::vmm::Vmm = memory::vmm::Vmm::new("kernel_lower_half");
static HIGHER_HALF: memory::vmm::Vmm = memory::vmm::Vmm::new("higher_half");

pub static CPU_DATA: SetOnce<CpuData> = SetOnce::new(CpuData::empty());
pub static FDT_PTR: Mutex<usize> = Mutex::new(0);
pub static KERN_PHYS: Mutex<usize> = Mutex::new(0);

#[thread_local]
pub static HART_ID: atomic::AtomicUsize = atomic::AtomicUsize::new(1);

pub const IO_OFFSET: u64 = 0xffffffff80000000;

/// # Safety
/// Should only be called once by the boot strap processor
pub unsafe fn init(map: &limine::MemoryMap, hhdm_start: u64, hart_id: usize, dtb: *const u8) {
    memory::HHDM_OFFSET.store(hhdm_start, Ordering::Relaxed);
    *FDT_PTR.lock() = dtb as usize;

    unsafe {
        memory::ALLOCATOR.lock().init(memory::HEAP.get() as usize, 16384);
    }
    memory::pmm::init(map);
    memory::init_tls();
    HART_ID.store(hart_id, core::sync::atomic::Ordering::Relaxed);
    println!("Hart ID: {hart_id}");
    memory::vmm::init();
    traps::init();

    unsafe {
        vmem::bootstrap()
    }

    let fdt = unsafe {fdt::Fdt::from_ptr(dtb).unwrap()};
    let fdt = alloc::boxed::Box::new(fdt);
    let fdt = alloc::boxed::Box::leak(fdt);

    let plic = fdt.find_node("/soc/plic@c000000").unwrap();
    let ptr = plic.reg().unwrap().next().unwrap().starting_address as usize;
    let ptr = ptr + hhdm_start as usize;

    traps::plic::PLIC_ADDR.store(ptr as *mut sifive_plic::Plic, Ordering::Relaxed);
    traps::plic_init();

    for node in fdt.all_nodes() {
        if node.name == "cpus" {
            let tps = node.property("timebase-frequency").unwrap().as_usize().unwrap();
            println!("\nClock runs at {}hz", tps);

            timing::TIMER_SPEED.store(tps as u64, Ordering::Relaxed);
        } else if node.name.contains("virtio") {
            let ptr = node.reg().unwrap().next().unwrap().starting_address as u64;
            let ptr = (ptr + IO_OFFSET) as *mut drivers::virtio::VirtIOHeader;
            
            unsafe {
                if (*ptr).is_valid() && ((*ptr).dev_id.read() != drivers::virtio::DeviceType::Reserved) {
                    println!("\nFound valid VirtIO {:?} device", (*ptr).dev_id.read());

                    let ints: Vec<usize> = node.interrupts().unwrap().collect();

                    drivers::virtio::VIRTIO_LIST.lock().push((AtomicPtr::new(ptr), ints.leak()));
                }
            }
        } else if node.name.contains("rtc") {
            println!("\nRTC Found {:#?}", node.name);

            let reg = node.reg().unwrap().next().unwrap().starting_address.add(IO_OFFSET as usize);
            let reg = reg as *mut drivers::goldfish_rtc::GoldfishRTC;

            drivers::goldfish_rtc::RTC.set(reg);
        } else if node.name.contains("pci") {
            println!("\nPCI host found");
            /*assert!(node.property("device_type").unwrap().as_str() == Some("pci"), "Not PCI bus");

            let bus_range = node.property("bus-range").unwrap();

            let ecam = node.reg().unwrap().next().unwrap();

            let bus_range = bus_range.value;
            let bus_start = u32::from_be_bytes(
                [
                    bus_range[0],
                    bus_range[1],
                    bus_range[2],
                    bus_range[3],
                ]
            ) as u8;
            let bus_end = u32::from_be_bytes(
                [
                    bus_range[4],
                    bus_range[5],
                    bus_range[6],
                    bus_range[7],
                ]
            ) as u8;

            let pci_host = drivers::pci::PCIHost {
                interrupt_map_mask: 0,
                interrupt_map: 0,
                ecam_region: drivers::pci::EcamRegion {
                    start: ecam.starting_address.add(IO_OFFSET as usize) as *mut u8,
                    size: ecam.size.unwrap()
                },
                bus_range: bus_start..=bus_end,
            };

            drivers::pci::PCI_HOST.set(pci_host);*/
        } else if node.name.contains("serial") {
            println!("Found serial");

            for int in node.interrupts().unwrap() {
                let plic = crate::traps::plic::PLIC_ADDR.load(core::sync::atomic::Ordering::Relaxed);

                (*plic).enable_interrupt(current_context(), int);
                (*plic).set_interrupt_priority(int, 0x2);

                crate::traps::plic::INT_HANDLERS.lock()[int] = uart::uart_handler;
            }
            
            uart::UART.lock().set_int();
        }
    }

    LOWER_HALF.add(0x1000, (hhdm_start - 0x1001) as usize).unwrap();

    for index in 256..512 {
        let entry = &(*memory::vmm::current_table()).0[index];
        let mut vaddr = memory::VirtualAddress(u64::MAX);
        let levels = memory::vmm::LEVELS.load(Ordering::Relaxed) as usize;
        let levels = memory::vmm::PageLevel::from_usize(levels);

        vaddr.set_index(levels, index as u64);
        if !entry.get_valid() {
            HIGHER_HALF.add(vaddr.0 as usize, levels.as_page_size() as usize).unwrap();
        }
    }

    println!("Vmem initialized");

    //drivers::virtio::init();
    //drivers::pci::init();

    // Test code for date code, as well as timing code
    /*let timestamp = (**drivers::goldfish_rtc::RTC.get()).time.read();
    println!("Raw timestamp: {}", timestamp);
    println!("date: {:?}", drivers::goldfish_rtc::UnixTimestamp(timestamp).date());
    timing::Unit::Seconds(20).set().unwrap();

    core::arch::riscv64::wfi();

    let timestamp = (**drivers::goldfish_rtc::RTC.get()).time.read();
    println!("Raw timestamp: {}", timestamp);
    println!("date: {:?}", drivers::goldfish_rtc::UnixTimestamp(timestamp).date());*/

    userspace::init_task_ids();
}

pub struct IOPtr<T>(*mut T)
    where T: Sized ;

unsafe impl<T> Send for IOPtr<T> {}
unsafe impl<T> Sync for IOPtr<T> {}

impl<T> IOPtr<T> {
    pub const fn new(ptr: *mut T) -> Self {
        Self(ptr)
    }
}

use core::{ops, sync::atomic::Ordering};

impl<T> ops::Deref for IOPtr<T> {
    type Target = T;
    
    fn deref(&self) -> &T {
        unsafe {
            &*self.0
        }
    }
}

impl<T> ops::DerefMut for IOPtr<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe {
            &mut *self.0
        }
    }
}

pub fn current_context() -> usize {
    let id = HART_ID.load(Ordering::Relaxed);

    // Assume we're on qemu
    1 + (2 * id)
}

bitflags::bitflags! {
    pub struct CpuData: u64 {
        /// Base isa, should always be true
        const I =           0b1;

        /// Multiplication extension
        const M =           0b10;

        /// Atomic extension
        const A =           0b100;

        /// Single precision floating point extension
        const F =           0b1000;

        /// Double precision floating point extension
        const D =           0b10000;

        /// Compressed instruction extension
        const C =           0b100000;

        /// TODO: Figure out meaning
        const ZICBOM =      0b1000000;

        /// TODO: Figure out meaning
        const ZICBOZ =      0b10000000;
        
        /// Control status register extension
        const ZICSR =       0b100000000;

        /// Instruction-fetch fence extension
        const ZIFENCEI =    0b1000000000;

        /// Pause hint extension
        const ZIHINTPAUSE = 0b10000000000;

        /// TODO: Figure out meaning
        const ZAWRS =       0b100000000000;

        /// TODO: Figure out meaning
        const ZBA =         0b1000000000000;

        /// TODO: Figure out meaning
        const ZBB =         0b10000000000000;

        /// TODO: Figure out meaning
        const ZBC =         0b100000000000000;

        /// TODO: Figure out meaning
        const ZBS =         0b1000000000000000;

        /// TODO: Figure out meaning
        const SSTC =        0b10000000000000000;

        /// TODO: Figure out meaning
        const SVADU =       0b100000000000000000;

        /// Page based memory type extension
        const SVPBMT =      0b1000000000000000000;
        
        /// Hypervisor Extension
        const H =           0b10000000000000000000;
    }
}

impl CpuData {
    pub fn parse_str(string: &str) -> Self {
        let string = string.trim_start_matches("rv64").chars().collect::<Vec<char>>();
        let mut newself = Self::empty();
        let mut single_char = true;

        let mut character_buffer = alloc::string::String::new();

        for character in string {
            if single_char {
                match character {
                    'i' => newself.set(CpuData::I, true),
                    'm' => newself.set(CpuData::M, true),
                    'a' => newself.set(CpuData::A, true),
                    'f' => newself.set(CpuData::F, true),
                    'd' => newself.set(CpuData::D, true),
                    'c' => newself.set(CpuData::C, true),
                    'h' => newself.set(CpuData::H, true),
                    '_' => single_char = false,
                    unknown_char => panic!("Unrecognized extension {:#?}", unknown_char)
                }
            } else {
                match character {
                    '_' => {
                        newself.set_from_str(&character_buffer);

                        character_buffer = alloc::string::String::new();
                    },
                    new_character => character_buffer.push(new_character)
                }
            }
        }

        newself.set_from_str(&character_buffer);

        newself
    }

    fn set_from_str(&mut self, string: &str) {
        match string {
            "zicbom" => self.set(CpuData::ZICBOM, true),
            "zicboz" => self.set(CpuData::ZICBOZ, true),
            "zicsr" => self.set(CpuData::ZICSR, true),
            "zifencei" => self.set(CpuData::ZIFENCEI, true),
            "zihintpause" => self.set(CpuData::ZIHINTPAUSE, true),
            "zawrs" => self.set(CpuData::ZAWRS, true),
            "zba" => self.set(CpuData::ZBA, true),
            "zbb" => self.set(CpuData::ZBB, true),
            "zbc" => self.set(CpuData::ZBC, true),
            "zbs" => self.set(CpuData::ZBS, true),
            "sstc" => self.set(CpuData::SSTC, true),
            "svadu" => self.set(CpuData::SVADU, true),
            "svpbmt" => self.set(CpuData::SVPBMT, true),
            ext => panic!("Unrecognized extension {:#?}", ext)
        }
    }
}

impl core::fmt::Debug for CpuData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.contains(CpuData::I) {
            writeln!(f, "\nBase extension")?;
        } else {
            panic!("Invalid extension set, base extension not set")
        }
        if self.contains(CpuData::M) {
            writeln!(f, "Multiply extension")?;
        }
        if self.contains(CpuData::A) {
            writeln!(f, "Atomic extension")?;
        }
        if self.contains(CpuData::C) {
            writeln!(f, "Compressed instruction extension")?;
        }
        if self.contains(CpuData::F) {
            writeln!(f, "Single precision floating point extension")?;
        }
        if self.contains(CpuData::H) {
            writeln!(f, "Hypervisor extension")?;
        }
        if self.contains(CpuData::D) {
            writeln!(f, "Double precision floating point extension")?;
        }
        if self.contains(CpuData::SSTC) {
            writeln!(f, "Unknown extension: 'SSTC'")?;
        }
        if self.contains(CpuData::ZAWRS) {
            writeln!(f, "Unknown extension: 'ZAWRS'")?;
        }
        if self.contains(CpuData::ZBA) {
            writeln!(f, "Unknown extension: 'ZBA'")?;
        }
        if self.contains(CpuData::ZBB) {
            writeln!(f, "Unknown extension: 'ZBB'")?;
        }
        if self.contains(CpuData::ZBC) {
            writeln!(f, "Unknown extension: 'ZBC'")?;
        }
        if self.contains(CpuData::ZBS) {
            writeln!(f, "Unknown extension: 'ZBS'")?;
        }
        if self.contains(CpuData::ZICBOM) {
            writeln!(f, "Unknown extension: 'ZICBOM'")?;
        }
        if self.contains(CpuData::ZICBOZ) {
            writeln!(f, "Unknown extension: 'ZICBOZ'")?;
        }
        if self.contains(CpuData::ZICSR) {
            writeln!(f, "Control status register extension")?;
        }
        if self.contains(CpuData::ZIFENCEI) {
            writeln!(f, "Instruction-fetch fence extension")?;
        }
        if self.contains(CpuData::ZIHINTPAUSE) {
            writeln!(f, "Pause hint extension")?;
        }
        if self.contains(CpuData::SVADU) {
            writeln!(f, "Unknown extension: 'SVADU'")?;
        }
        if self.contains(CpuData::SVPBMT) {
            writeln!(f, "Page based memory type extension")?;
        }

        Ok(())
    }
}

pub struct SetOnce<T: ?Sized> {
    set: AtomicBool,
    val: UnsafeCell<T>,
}

impl<T> SetOnce<T> {
    pub const fn new(base_val: T) -> Self {
        Self {
            set: AtomicBool::new(false),
            val: UnsafeCell::new(base_val),
        }
    }

    pub fn set(&self, new_val: T) {
        if self.set.load(Ordering::Relaxed) {
            panic!("Attempted to write to a setonce cell");
        }

        self.set.store(true, Ordering::Relaxed);
        unsafe {
            *self.val.get() = new_val;
        }
    }

    pub fn get(&self) -> &T {
        unsafe {
            &*self.val.get()
        }
    }
}

unsafe impl<T> Send for SetOnce<T> {}
unsafe impl<T> Sync for SetOnce<T> {}