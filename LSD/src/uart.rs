// SPDX-FileCopyrightText: © 2023 Archaic Archea <archaic.archea@gmail.com>
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use spin::Mutex;

use crate::{volatile::Volatile, IOPtr};

pub static UART: Mutex<IOPtr<Uart16550>> = Mutex::new(IOPtr::new(0x1000_0000 as *mut Uart16550));

#[repr(C)]
pub struct Uart16550 {
    data_register: Volatile<u8>,
    interrupt_enable: Volatile<u8>,
    int_id_fifo_control: Volatile<u8>,
    line_control: Volatile<u8>,
    modem_control: Volatile<u8>,
    line_status: Volatile<u8>,
    modem_status: Volatile<u8>,
    scratch: Volatile<u8>,
}

use core::fmt;

impl fmt::Write for Uart16550 {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for character in s.chars() {
            self.data_register.write(character as u8);
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
    ($($arg:tt)*) => ($crate::uart::_print(format_args!($($arg)*)));
}
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    UART.lock().write_fmt(args).unwrap();
}