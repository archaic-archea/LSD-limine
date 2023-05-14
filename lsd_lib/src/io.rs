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

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::io::_print(format_args!($($arg)*)));
}
#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    let mut writer = RootPrinter;
    writer.write_fmt(args).unwrap();
}