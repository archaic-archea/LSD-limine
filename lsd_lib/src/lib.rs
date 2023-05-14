#![no_std]
#![feature(core_intrinsics)]

pub extern crate alloc as alloc_core;

pub mod alloc;
pub mod time;
pub mod raw_calls;
pub mod thread;
pub mod io;

pub use alloc_core::{boxed, borrow};