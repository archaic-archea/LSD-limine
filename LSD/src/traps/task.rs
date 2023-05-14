use core::sync::atomic::{Ordering, AtomicUsize};

use alloc::vec::Vec;
use spin::RwLock;

pub static TASK_QUEUES: RwLock<Vec<RwLock<TaskQueue>>> = RwLock::new(Vec::new());

#[thread_local]
pub static TASK_LOCK_INDEX: AtomicUsize = AtomicUsize::new(0);

pub fn update_current(frame: &mut super::TrapFrame) {
    let task_queues = TASK_QUEUES.read();
    let mut lock = task_queues[TASK_LOCK_INDEX.load(Ordering::Relaxed)].write();

    let current = lock.current_task_mut();
    current.trap_frame = *frame;
}

pub fn advance_task(frame: &mut super::TrapFrame) {
    let task_queues = TASK_QUEUES.read();
    let mut lock = task_queues[TASK_LOCK_INDEX.load(Ordering::Relaxed)].write();

    if lock.cur_task_idx >= lock.queue.len() {
        lock.cur_task_idx = 0;
    }

    let current = lock.current_task_mut();

    current.trap_frame = *frame;

    lock.advance();
    let new_task = lock.current_task();

    unsafe {
        let new_satp = new_task.task_table.0;
        
        core::arch::asm!(
            "csrw satp, {new_satp}",
            new_satp = in(reg) new_satp
        );
        core::arch::asm!("sfence.vma");
    }

    *frame = new_task.trap_frame;
}

pub fn new_universal_task(task_data: TaskData) {
    let lock = TASK_QUEUES.read();

    for queue in lock.iter() {
        let mut write = queue.write();

        write.new_task(task_data);
    }
}

pub fn new_task(task_data: TaskData) {
    let lock = TASK_QUEUES.read();
    
    let mut lowest_idx = usize::MAX;

    for (index, queue) in lock.iter().enumerate() {
        let read = queue.read();

        if read.queue.len() < lowest_idx {
            lowest_idx = index;
        }
    }

    lock[lowest_idx].write().new_task(task_data);
}

pub fn drop_task(task_index: usize) {
    let task_queues = TASK_QUEUES.read();
    let mut lock = task_queues[TASK_LOCK_INDEX.load(Ordering::Relaxed)].write();

    if lock.cur_task_idx == task_index {
        lock.advance();
    }

    lock.queue.swap_remove(task_index);
    lock.cur_task_idx -= 1;
}

pub fn full_drop_task(_index: usize) {
    let task_queues = TASK_QUEUES.read();
    let mut lock = task_queues[TASK_LOCK_INDEX.load(Ordering::Relaxed)].write();

    let cur = lock.current_task();
    let id = cur.task_id;
    while lock.current_task().task_id == id {
        lock.advance();
    }

    unsafe {lock.current_task().task_table.set()}
    let mut indexes = alloc::vec::Vec::new();

    for (index, task) in lock.queue.iter().rev().enumerate() {
        let table_addr = (task.task_table.get_ppn() << 12) + crate::memory::HHDM_OFFSET.load(core::sync::atomic::Ordering::Relaxed);
        let table = table_addr as *mut crate::memory::vmm::PageTable;
        unsafe {(*table).destroy_completely()}

        indexes.push(index);
    }

    for index in indexes.iter() {
        lock.queue.swap_remove(*index);
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u64)]
pub enum Privilege {
    Root        = 0xffff_ffff,
    SuperUser   = 0xffff,
    User        = 0xff,
    Guest       = 0xf
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct TaskData {
    pub trap_frame: super::TrapFrame,
    pub task_id: usize,
    pub task_table: crate::memory::vmm::Satp,
    pub privilege: Privilege,
    pub waiting_on: WaitSrc,
    pub thread_id: usize,
    pub thread_manager: &'static vmem::Vmem<'static, 'static>,
    pub vmm: &'static vmem::Vmem<'static, 'static>
}

impl TaskData {
    /// # Safety
    /// Mostly safe, but will not return, handle initialization of everything before running this
    #[naked]
    pub unsafe extern "C" fn load(&self) -> !{
        // a0 starts loaded with a pointer to self, that we will treat as a pointer to its trap frame
        core::arch::asm!(
            "
                // Load sepc
                ld t0, 0(a0)
                csrw sepc, t0

                // Load satp
                ld t0, 264(a0)
                csrw satp, t0

                sfence.vma

                // Load registers
                ld x1, 8(a0)
                ld x2, 16(a0)
                ld x3, 24(a0)
                ld x4, 32(a0)
                ld x5, 40(a0)
                ld x6, 48(a0)
                ld x7, 56(a0)
                ld x8, 64(a0)
                ld x9, 72(a0)
                ld x11, 88(a0)
                ld x12, 96(a0)
                ld x13, 104(a0)
                ld x14, 112(a0)
                ld x15, 120(a0)
                ld x16, 128(a0)
                ld x17, 136(a0)
                ld x18, 144(a0)
                ld x19, 152(a0)
                ld x20, 160(a0)
                ld x21, 168(a0)
                ld x22, 176(a0)
                ld x23, 184(a0)
                ld x24, 192(a0)
                ld x25, 200(a0)
                ld x26, 208(a0)
                ld x27, 216(a0)
                ld x28, 224(a0)
                ld x29, 232(a0)
                ld x30, 240(a0)
                ld x31, 248(a0)

                ld x10, 80(a0)

                sret
            ", 
            options(noreturn)
        )
    }
}

pub struct TaskQueue {
    pub queue: Vec<TaskData>,
    pub cur_task_idx: usize,
}

impl TaskQueue {
    pub const fn new() -> Self {
        Self {
            queue: Vec::new(),
            cur_task_idx: 0,
        }
    }

    pub fn advance(&mut self) {
        let start = self.cur_task_idx;

        loop {
            self.cur_task_idx += 1;
            if self.cur_task_idx == self.queue.len() {
                self.cur_task_idx = 0;
            }

            if self.current_task().waiting_on == WaitSrc::None {
                break;
            } else if let WaitSrc::Time(ts) = self.current_task().waiting_on {
                let rtc = unsafe {(**crate::drivers::goldfish_rtc::RTC.get()).time.read()};

                if ts <= rtc {
                    self.current_task_mut().waiting_on = WaitSrc::None;
                    break;
                }
            } else if let WaitSrc::TaskEnd(task_id, thread_id_opt) = self.current_task().waiting_on {
                let task_found = match thread_id_opt {
                    Some(thread_id) => {
                        let task = self.queue.iter().find(|task| {
                            (task.task_id == task_id) && (task.thread_id == thread_id.get())
                        });

                        task.is_some()
                    },
                    None => {
                        let task = self.queue.iter().find(|task| {
                            task.task_id == task_id
                        });

                        task.is_some()
                    }
                };

                if !task_found {
                    self.current_task_mut().waiting_on = WaitSrc::None;
                    break;
                }
            }

            if self.cur_task_idx == start {
                panic!("No task available");
            }
        }
    }

    pub fn current_task(&self) -> &TaskData {
        &self.queue[self.cur_task_idx]
    }

    pub fn current_task_mut(&mut self) -> &mut TaskData {
        &mut self.queue[self.cur_task_idx]
    }

    pub fn new_task(&mut self, task_data: TaskData) {
        self.queue.push(task_data);
    }

    pub fn find_task(&self, id: usize) -> Option<&TaskData> {
        self.queue.iter().find(|entry| entry.task_id == id)
    }

    pub fn find_task_mut(&mut self, id: usize) -> Option<&mut TaskData> {
        self.queue.iter_mut().find(|entry| entry.task_id == id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WaitSrc {
    None,
    CharIn,
    Breakpoint,
    /// Waiting on a certain time stamp to be reached
    Time(u64),

    /// Waiting on a task/thread to end
    TaskEnd(usize, Option<core::num::NonZeroUsize>)
}

impl core::fmt::Debug for TaskData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "Task id: 0x{:x}", self.task_id)?;
        writeln!(f, "Thread id: 0x{:x}", self.thread_id)?;

        Ok(())
    }
}