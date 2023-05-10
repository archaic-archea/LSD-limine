#![no_std]
#![feature(core_intrinsics)]

struct RootPrinter;

impl core::fmt::Write for RootPrinter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let ptr = bytes.as_ptr();
        let len = bytes.len();
    
        unsafe {
            core::arch::asm!(
                "ecall",
                in("a0") 0,
                in("a1") 1,
                in("a2") ptr,
                in("a3") len,
            );
        }

        Ok(())
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

/// Spawns a new thread, can damage memory if not handled well
/// Will also return task ID in `a0`, and thread ID in `a1`
/// # Safety
/// Always call `drop_thread` when done with the thread
pub unsafe fn spawn_thread_raw() -> (usize, usize) {
    let task_id: usize;
    let thread_id: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            in("a0") 1,
            in("a1") 2,
            lateout("a0") task_id,
            lateout("a1") thread_id,
        );
    }

    (task_id, thread_id)
}

/// Safely spawns a thread, ensuring it is dropped at the end of it's lifetime
pub fn spawn_thread(f: fn()) {
    let ids = unsafe {spawn_thread_raw()};

    if ids.1 != 0 {
        f();
        unsafe {drop_thread()};
    }
}

/// Causes a thread to stop executing, should only be called at the end of  athread
/// # Safety
/// Only safe when ran at the end of a thread after everything it uses has been dropped
pub unsafe fn drop_thread() -> ! {
    core::arch::asm!(
        "ecall",
        in("a0") 1,
        in("a1") 3,
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


#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::_print(format_args!($($arg)*)));
}
#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    let mut writer = RootPrinter;
    writer.write_fmt(args).unwrap();
}