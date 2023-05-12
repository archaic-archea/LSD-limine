pub fn current_ts() -> u64 {
    let out: u64;

    unsafe {
        core::arch::asm!(
            "ecall",
            in("a0") 1,
            in("a1") 5,
            in("a2") 0,
            lateout("a0") out,
        );
    }

    out
}

pub fn await_ts(ts: u64) {
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a0") 1,
            in("a1") 5,
            in("a2") ts,
        );
    }
}

/// Spawns a new thread, can damage memory if not handled well
/// Will also return task ID in `a0`, and thread ID in `a1`
/// # Safety
/// Always call `drop_thread` when done with the thread
pub unsafe fn spawn_thread_raw() -> (usize, usize, bool) {
    let task_id: usize;
    let thread_id: usize;
    let is_thread: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            in("a0") 1,
            in("a1") 2,
            lateout("a0") task_id,
            lateout("a1") thread_id,
            lateout("a2") is_thread
        );
    }

    let is_thread = is_thread != 0;

    (task_id, thread_id, is_thread)
}

/// Causes a thread to stop executing, should only be called at the end of  athread
/// # Safety
/// Only safe when ran at the end of a thread after everything it uses has been dropped
pub unsafe fn drop_thread<T>(val: *mut T) -> ! {
    core::arch::asm!(
        "ecall",
        in("a0") 1,
        in("a1") 3,
        in("a2") val,
        options(noreturn)
    );
}

/// Forfeits control to the next task immediately rather waiting on an IO call, or a timed switch
pub fn forfeit() {
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a0") 1,
            in("a1") 0,
        );
    }
}

pub fn extend_heap(size: usize) -> *mut u8 {
    let out: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            in("a0") 1,
            in("a1") 1,
            in("a2") size,
            lateout("a0") out
        );
    }

    out as *mut u8
}

pub fn in_char() -> char {
    let out: u32;

    unsafe {
        core::arch::asm!(
            "ecall",
            in("a0") 0,
            in("a1") 2,
            lateout("a0") out
        );
    }

    unsafe {char::from_u32_unchecked(out)}
}

/// Pause this thread/task until the target task/thread cannot be found
/// A thread id of 0 indicates that this is not searching for a specific thread, but any thread with the task id given
/// # Safety
/// Only safe if you KNOW the task will end
/// 
/// # Unsafety
/// Awaiting on tasks that will not end, such as task (0, 0) (the idle task) will cause this function to never return
pub fn await_task_end<T>(task_id: usize, thread_id: usize) -> *mut T {
    let ptr: *mut T;

    unsafe {
        core::arch::asm!(
            "ecall",
            in("a0") 1,
            in("a1") 6,
            in("a2") task_id,
            in("a3") thread_id,
            lateout("a0") ptr
        );
    }

    ptr
}