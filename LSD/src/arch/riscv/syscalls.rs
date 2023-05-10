// [0][1][ptr][len]            = print string               -> no return
// [0][2]                      = take input                 -> [char]
// 
// [1][0]                      = forfeit task control       -> no return
// [1][1][size]                = extend heap                -> [ptr]
// [1][2]                      = spawn thread               -> [task_id][thread_id]
// [1][3]                      = drop current thread        -> no return
// [1][4]                      = End program                -> no return

use alloc::vec::Vec;
use spin::Mutex;

use crate::{print, println};

pub fn syscall_core(trap_frame: &mut crate::traps::TrapFrame) {
    match trap_frame.a0 {
        0 => kernel_io(trap_frame),
        1 => kernel_task(trap_frame),
        call =>  panic!("Unrecognized syscall root 0x{:x} trapframe: \n{:#x?}", call, trap_frame)
    }
}

/// Contains a vector of task IDs waiting on an input character
pub static INPUT_AWAIT_LIST: Mutex<Vec<usize>> = Mutex::new(Vec::new());

pub fn kernel_io(trap_frame: &mut crate::traps::TrapFrame) {
    use crate::memory::{vmm, self};

    match trap_frame.a1 {
        1 => {
            unsafe {
                let virt = trap_frame.a2 as u64;
                let phys = vmm::virt_to_phys(memory::VirtualAddress(virt)).unwrap();
                let ptr = phys.as_ptr();
                let len = trap_frame.a3;

                for i in 0..len {
                    print!("{}", *ptr.add(i) as char);
                }
            }
        },
        2 => {
            use crate::traps::task;

            INPUT_AWAIT_LIST.lock().push(
                task::CURRENT_USER_TASK.read().current_task().task_id
            );
            task::CURRENT_USER_TASK.write().current_task_mut().waiting_on = task::WaitSrc::CharIn;
            task::advance_task(trap_frame);
        },
        subcall => panic!("Unrecognized io subcall 0x{:x} trapframe: \n{:#x?}", subcall, trap_frame)
    }
}

pub fn kernel_task(trap_frame: &mut crate::traps::TrapFrame) {
    match trap_frame.a1 {
        0 => crate::traps::task::advance_task(trap_frame),
        1 => {
            use crate::memory::{self, pmm, vmm};

            let bytes = trap_frame.a2;
            let frames = bytes.div_ceil(0x1000);

            let mut claims = (0..frames).map(|_| {
                pmm::REGION_LIST.lock().claim() as u64 - memory::HHDM_OFFSET.load(core::sync::atomic::Ordering::Relaxed)
            });

            let read = crate::traps::task::CURRENT_USER_TASK.read();

            let cur_task = read.current_task();
            let vaddr = cur_task.vmm.alloc(frames * 0x1000, vmem::AllocStrategy::NextFit).unwrap() as u64;

            trap_frame.a0 = vaddr as usize;
            
            let vaddrs = (vaddr..vaddr + (frames as u64) * 0x1000).step_by(0x1000);

            let table = (cur_task.task_table.get_ppn() << 12) + memory::HHDM_OFFSET.load(core::sync::atomic::Ordering::Relaxed);
            let table = table as *mut vmm::PageTable;
            println!("extending heap on current task");

            for vaddr in vaddrs {
                let paddr = claims.next().unwrap();

                let virt = memory::VirtualAddress(vaddr);
                let phys = memory::PhysicalAddress(paddr);
                
                let mut lock = pmm::REGION_LIST.lock();

                let flags = vmm::PageFlags::READ | vmm::PageFlags::WRITE | vmm::PageFlags::USER;

                let level = vmm::LEVELS.load(core::sync::atomic::Ordering::Relaxed) as usize;
                let level = vmm::PageLevel::from_usize(level);

                unsafe {
                    vmm::map(
                        table, 
                        virt, 
                        phys, 
                        level, 
                        vmm::PageLevel::Level1, 
                        &mut lock, 
                        flags
                    );

                    core::arch::asm!("sfence.vma");
                }
            }
        },
        2 => {
            crate::traps::task::update_current(trap_frame);
            let mut read = crate::traps::task::CURRENT_USER_TASK.write();

            let cur_task = read.current_task_mut();
            trap_frame.a0 = 0;

            let mut task_clone = *cur_task;
            task_clone.thread_id = task_clone.thread_manager.alloc(1, vmem::AllocStrategy::NextFit).unwrap();
            task_clone.trap_frame.a0 = task_clone.task_id;
            task_clone.trap_frame.a1 = task_clone.thread_id;

            core::mem::drop(read);
            crate::traps::task::new_task(task_clone);
        },
        3 => {
            let read = crate::traps::task::CURRENT_USER_TASK.read();
            let index = read.cur_task_idx;
            core::mem::drop(read);

            crate::traps::task::advance_task(trap_frame);

            crate::traps::task::drop_task(index);
        },
        4 => {
            let read = crate::traps::task::CURRENT_USER_TASK.read();
            let current = read.current_task();
            let id = current.task_id;
            core::mem::drop(read);

            crate::traps::task::advance_task(trap_frame);

            let write = crate::traps::task::CURRENT_USER_TASK.write();

            for (index, entry) in write.queue.iter().rev().enumerate() {
                if entry.task_id == id {
                    if entry.thread_id == 0 {
                        crate::traps::task::full_drop_task(index);
                    } else {
                        crate::traps::task::drop_task(index)
                    }
                }
            }
        },
        subcall => panic!("Unrecognized task subcall 0x{:x} trapframe: \n{:#x?}", subcall, trap_frame)
    }
}
