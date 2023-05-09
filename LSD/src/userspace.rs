use spin::Mutex;

use crate::println;

pub static TASK_IDS: Mutex<vmem::Vmem> = Mutex::new(
    vmem::Vmem::new(
        alloc::borrow::Cow::Borrowed("TaskIDs"), 
        2, 
        None
    )
);

pub fn init_task_ids() {
    TASK_IDS.lock().add(0, usize::MAX).unwrap();
}

pub fn start_tasks() -> ! {
    let lock = crate::traps::task::CURRENT_USER_TASK.read();
    let task = *lock.current_task();

    core::mem::drop(lock);

    unsafe {
        println!("Jumping to userspace");
        task.load();
    }
}

pub fn load(bytes: &[u8]) -> usize {
    use crate::memory::{pmm, vmm, self};

    let elfbytes = elf::ElfBytes::<elf::endian::LittleEndian>::minimal_parse(bytes).unwrap();
    let table = elfbytes.segments().unwrap();

    let new_table = vmm::new_with_upperhalf();
    let level = vmm::PageLevel::from_usize(vmm::LEVELS.load(core::sync::atomic::Ordering::Relaxed) as usize);

    for entry in table {
        // If type is LOAD, load it into memory
        if entry.p_type == 0x1 {
            let mut frames = entry.p_memsz / 4096;
            if (entry.p_memsz % 4096) != 0 {
                frames += 1;
            }

            let current_entry = pmm::REGION_LIST.lock().claim_continuous(frames as usize).unwrap();

            if entry.p_filesz == 0 {
                for i in 0..entry.p_memsz as usize {
                    unsafe {
                        *current_entry.add(i) = 0;
                    }
                }
            }

            for i in 0..entry.p_filesz as usize {
                unsafe {
                    *current_entry.add(i) = bytes[i + entry.p_offset as usize];
                }
            }

            let flags = Flags::from_bits_retain(entry.p_flags);
            let flags = flags.to_pageflags();
            let flags = flags | vmm::PageFlags::USER;

            let base_phys = (current_entry as u64) - memory::HHDM_OFFSET.load(core::sync::atomic::Ordering::Relaxed);
            let base_virt = entry.p_vaddr;

            for offset in (0..entry.p_memsz).step_by(4096) {
                let phys = memory::PhysicalAddress(base_phys + offset);
                let virt = memory::VirtualAddress(base_virt + offset);

                unsafe {
                    vmm::map(
                        new_table, 
                        virt, 
                        phys, 
                        level, 
                        vmm::PageLevel::Level1, 
                        &mut pmm::REGION_LIST.lock(), 
                        flags
                    );
                }
            }
        }
    }

    let task_id = TASK_IDS.lock().alloc(0x1, vmem::AllocStrategy::NextFit).unwrap();

    let task_vmm = vmem::Vmem::new(
        alloc::borrow::Cow::Borrowed("task_vmm"), 
        4096, 
        None
    );

    for i in 1..=256 {
        unsafe {
            let entry = &(*new_table).0[i];
            let mut vaddr = memory::VirtualAddress(0);
            vaddr.set_index(level, i as u64);

            if !entry.get_valid() {
                task_vmm.add(
                    vaddr.0 as usize, 
                    vmm::PageSize::from_level(level) as usize
                ).unwrap()
            }
        }
    }

    let stack = pmm::REGION_LIST.lock().claim_aligned(0x800, vmm::PageSize::Medium).unwrap();
    let stack_paddr = (stack as u64) - memory::HHDM_OFFSET.load(core::sync::atomic::Ordering::Relaxed);
    let stack_vaddr = task_vmm.alloc(0x80_0000, vmem::AllocStrategy::NextFit).unwrap() as u64;

    println!("Physical address of new stack: 0x{:x}", stack_paddr);

    for i in (0..0x80_0000).step_by(vmm::PageSize::Medium as usize) {
        let flags = vmm::PageFlags::READ | vmm::PageFlags::WRITE | vmm::PageFlags::USER;

        let virt = memory::VirtualAddress(stack_vaddr + i);
        let phys = memory::PhysicalAddress(stack_paddr + i);
        unsafe {
            println!("Mapping 0x{:x} to 0x{:x}", virt.0, phys.0);
            vmm::map(
                new_table, 
                virt, 
                phys, 
                level, 
                vmm::PageLevel::Level2, 
                &mut pmm::REGION_LIST.lock(), 
                flags
            )
        }
    }

    use crate::traps::{self, task};

    let phys = (new_table as u64) - memory::HHDM_OFFSET.load(core::sync::atomic::Ordering::Relaxed);

    let mut task_table = vmm::Satp::new();
    task_table.set_asid(0);
    task_table.set_mode(vmm::PageType::from_levels(level) as u64);
    task_table.set_ppn(phys >> 12);
    
    let boxed_vmm = alloc::boxed::Box::new(task_vmm);
    let leaked_vmm = alloc::boxed::Box::leak(boxed_vmm);

    let task_tm = vmem::Vmem::new(
        alloc::borrow::Cow::Borrowed("task_thread_manager"), 
        1, 
        None
    );

    task_tm.add(0, usize::MAX).unwrap();

    let boxed_tm = alloc::boxed::Box::new(task_tm);
    let leaked_tm = alloc::boxed::Box::leak(boxed_tm);

    let mut task_data = task::TaskData {
        trap_frame: traps::TrapFrame::default(),
        task_id,
        task_table,
        privilege: task::Privilege::User,
        waiting_on: task::WaitSrc::None,
        thread_id: leaked_tm.alloc(1, vmem::AllocStrategy::NextFit).unwrap(),
        thread_manager: leaked_tm,
        vmm: leaked_vmm
    };

    task_data.trap_frame.sp = stack_vaddr as usize;
    println!("Loading program with stack at {:?}", task_data.trap_frame.sp());

    task_data.trap_frame.sepc = elfbytes.ehdr.e_entry as usize;
    task_data.trap_frame.a0 = task_id;

    task::new_task(task_data);
    task_id
}

bitflags::bitflags! {
    struct Flags: u32 {
        const EXECUTE = 0b00000001;
        const WRITE = 0b00000010;
        const READ = 0b00000100;
    }
}

impl Flags {
    pub fn to_pageflags(&self) -> crate::memory::vmm::PageFlags {
        use crate::memory::vmm::PageFlags;

        let mut flags = PageFlags::empty();

        if self.contains(Flags::EXECUTE) {
            flags |= PageFlags::EXECUTE;
        }
        if self.contains(Flags::WRITE) {
            flags |= PageFlags::WRITE;
        }
        if self.contains(Flags::READ) {
            flags |= PageFlags::READ;
        }

        flags
    }
}