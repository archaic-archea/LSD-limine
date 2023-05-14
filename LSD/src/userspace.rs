use spin::Mutex;

use crate::println;

pub static TASK_IDS: Mutex<vmem::Vmem> = Mutex::new(
    vmem::Vmem::new(
        alloc::borrow::Cow::Borrowed("TaskIDs"), 
        1, 
        None
    )
);

/// # Safety
/// Only call once per core
pub unsafe fn init_task_queues() {
    crate::traps::task::TASK_QUEUES.write().push({
        let task_queue = spin::RwLock::new(
            crate::traps::task::TaskQueue::new()
        );

        task_queue.write().new_task(
            load(crate::NULL_TASK, crate::traps::task::Privilege::Guest)
        );
        
        task_queue
    });
}

pub fn init_task_ids() {
    TASK_IDS.lock().add(0, usize::MAX).unwrap();
}

pub fn start_tasks() {
    crate::traps::task::TASK_LOCK_INDEX.store(0, core::sync::atomic::Ordering::Relaxed);
    let task_queues = crate::traps::task::TASK_QUEUES.read();
    let lock = task_queues[crate::traps::task::TASK_LOCK_INDEX.load(core::sync::atomic::Ordering::Relaxed)].read();
    let task = *lock.current_task();

    core::mem::drop(lock);

    unsafe {
        println!("Jumping to userspace");
        task.load();
    }
}

pub fn load(bytes: &[u8], privilege: crate::traps::task::Privilege) -> crate::traps::task::TaskData {
    use crate::memory::{pmm, vmm, self};

    let elfbytes = elf::ElfBytes::<elf::endian::LittleEndian>::minimal_parse(bytes).unwrap();
    let table = elfbytes.segments().unwrap();

    let new_table = vmm::new_with_upperhalf();
    let level = vmm::PageLevel::from_usize(vmm::LEVELS.load(core::sync::atomic::Ordering::Relaxed) as usize);

    for entry in table {
        // If type is LOAD, load it into memory
        if entry.p_type == 0x1 {
            for (page_offset, vaddr) in (entry.p_vaddr..entry.p_vaddr + entry.p_memsz).step_by(0x1000).enumerate() {
                let physical = pmm::REGION_LIST.lock().claim();

                if entry.p_filesz == 0 {
                    for i in 0..4096 {
                        unsafe {
                            *physical.add(i) = 0;
                        }
                    }
                } else {
                    for i in 0..4096 {
                        unsafe {
                            *physical.add(i) = bytes[i + page_offset + entry.p_offset as usize];
                        }
                    }
                }

                let flags = Flags::from_bits_retain(entry.p_flags);
                let flags = flags.to_pageflags();
                let flags = flags | vmm::PageFlags::USER;

                let paddr = (physical as u64) - memory::HHDM_OFFSET.load(core::sync::atomic::Ordering::Relaxed);

                let phys = memory::PhysicalAddress(paddr);
                let virt = memory::VirtualAddress(vaddr);

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

    for i in 2..=256 {
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
    let stack_vaddr = task_vmm.alloc(0x80_0001, vmem::AllocStrategy::NextFit).unwrap() as u64;

    for i in (0..0x80_0000).step_by(vmm::PageSize::Medium as usize) {
        let flags = vmm::PageFlags::READ | vmm::PageFlags::WRITE | vmm::PageFlags::USER;

        let virt = memory::VirtualAddress(stack_vaddr + i);
        let phys = memory::PhysicalAddress(stack_paddr + i);
        unsafe {
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

    task_tm.add(1, usize::MAX - 1).unwrap();

    let boxed_tm = alloc::boxed::Box::new(task_tm);
    let leaked_tm = alloc::boxed::Box::leak(boxed_tm);

    let mut task_data = task::TaskData {
        trap_frame: traps::TrapFrame::default(),
        task_id,
        task_table,
        privilege,
        waiting_on: task::WaitSrc::None,
        thread_id: leaked_tm.alloc(1, vmem::AllocStrategy::NextFit).unwrap(),
        thread_manager: leaked_tm,
        vmm: leaked_vmm
    };

    task_data.trap_frame.sp = (stack_vaddr + 0x80_0000) as usize;
    println!("Loading program with stack at {:?}", task_data.trap_frame.sp());

    task_data.trap_frame.sepc = elfbytes.ehdr.e_entry as usize;
    task_data.trap_frame.a0 = task_id;
    
    task_data
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