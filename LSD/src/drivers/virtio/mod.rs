/// Most of the VirtIO code is derived from the Vanadinite project's code, which is licensed under MPL-2.0
/// Visit `https://github.com/repnop/vanadinite` for more information on the project, as well as license

use core::sync::atomic::AtomicPtr;

use alloc::vec::Vec;
use spin::Mutex;

use crate::{volatile::*, println, current_context};

pub mod splitqueue;
pub mod input;
pub mod entropy;

pub static VIRTIO_LIST: Mutex<Vec<(AtomicPtr<VirtIOHeader>, &mut [usize])>> = Mutex::new(Vec::new());

/// # Safety
/// Only call once when all virtio devices have been read
pub unsafe fn init() {
    for device_atom_ptr in VIRTIO_LIST.lock().iter() {
        let device_ptr = device_atom_ptr.0.load(core::sync::atomic::Ordering::Relaxed);
        let slice = &device_atom_ptr.1;
        match (*device_ptr).dev_id.read() {
            DeviceType::Input => {
                println!("Found input device");

                let plic = crate::traps::plic::PLIC_ADDR.load(core::sync::atomic::Ordering::Relaxed);

                for int in slice.iter() {
                    println!("Found input interrupt 0x{:x}", int);
                    (*plic).enable_interrupt(current_context(), *int);
                    (*plic).set_interrupt_priority(*int, 0x2);

                    crate::traps::plic::INT_HANDLERS.lock()[*int] = input::handle_int;
                }

                input::init(device_ptr);
            },
            DeviceType::Entropy => {
                println!("Found entropy device at {:?}", device_ptr);

                let plic = crate::traps::plic::PLIC_ADDR.load(core::sync::atomic::Ordering::Relaxed);

                for int in slice.iter() {
                    println!("Found entropy interrupt 0x{:x}", int);
                    (*plic).enable_interrupt(current_context(), *int);
                    (*plic).set_interrupt_priority(*int, 0x2);

                    //crate::traps::plic::INT_HANDLERS.lock()[*int] = ;
                }

                let entropy = entropy::Entropy::init(device_ptr);
                entropy.request(16);

                panic!("Entropy over")
            },
            dev_type => {
                println!("Unsupported device type {:?}", dev_type);
            }
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct VirtIOHeader {
    pub magic: Volatile<u32, Read>, // 0x00
    pub version: Volatile<u32, Read>, // 0x04
    pub dev_id: Volatile<DeviceType, Read>, // 0x08
    pub vend_id: Volatile<u32, Read>, // 0x0C

    pub dev_feat: Volatile<u32, Read>, // 0x10
    pub dev_feat_sel: Volatile<u32, Write>, // 0x14

    _reserved1: [u32; 2], // 0x18

    pub driver_feat: Volatile<u32, Write>, // 0x20
    pub driver_feat_sel: Volatile<u32, Write>, // 0x24

    _reserved2: [u32; 2], // 0x28

    pub queue_sel: Volatile<u32, Write>, // 0x30
    pub queue_size_max: Volatile<u32, Read>, // 0x34
    pub queue_size: QueueSize, // 0x38
    _reserved3: [u32; 2], // 0x3C
    pub queue_ready: QueueReady, // 0x44
    _reserved4: [u32; 2], // 0x48
    pub queue_notify: QueueNotify, // 0x50

    _reserved5: [u32; 3], // 0x54

    pub int_status: Volatile<u32, Read>, // 0x60
    pub int_ack: IntAck, // 0x64

    _reserved6: [u32; 2], // 0x68

    pub status: Status, // 0x70

    _reserved7: [u32; 3], // 0x74

    pub queue_desc: QueueDescriptor, // 0x80

    _reserved8: [u32; 2], // 0x88

    pub queue_avail: QueueAvailable, // 0x90

    _reserved9: [u32; 2], // 0x98

    pub queue_used: QueueUsed, // 0xA0

    _reserved10: [u32; 21], // 0xA8

    pub config_gen: Volatile<u32, Read> //0xFC
}

impl VirtIOHeader {
    pub fn is_valid(&self) -> bool {
        self.magic.read() == 0x74726976
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u32)]
pub enum DeviceType {
    Reserved = 0,
    Network = 1,
    Block = 2,
    Console = 3,
    Entropy = 4,
    TradMemBalloon = 5,
    SCSIHost = 8,
    GPU = 16,
    Input = 18,
    Socket = 19,
    Cryptography = 20,
}

#[repr(transparent)]
#[derive(Debug)]
pub struct QueueSize(Volatile<u32, Write>);

impl QueueSize {
    pub fn write(&self, val: u32) {
        println!("Writing queue size {}", val);
        self.0.write(val);
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct QueueDescriptor(Volatile<[u32; 2], ReadWrite>);

impl QueueDescriptor {
    pub fn set(&self, addr: crate::memory::PhysicalAddress) {
        println!("Storing descriptor queue at 0x{:x}", addr.0);
        let low = (addr.0 & 0xFFFF_FFFF) as u32;
        let high = (addr.0 >> 32) as u32;
        self.0[0].write(low);
        self.0[1].write(high);
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct QueueAvailable(Volatile<[u32; 2], ReadWrite>);

impl QueueAvailable {
    pub fn set(&self, addr: crate::memory::PhysicalAddress) {
        println!("Storing available queue at 0x{:x}", addr.0);
        let low = (addr.0 & 0xFFFF_FFFF) as u32;
        let high = (addr.0 >> 32) as u32;
        self.0[0].write(low);
        self.0[1].write(high);
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct QueueUsed(Volatile<[u32; 2], ReadWrite>);

impl QueueUsed {
    pub fn set(&self, addr: crate::memory::PhysicalAddress) {
        println!("Storing used queue at 0x{:x}", addr.0);
        let low = (addr.0 & 0xFFFF_FFFF) as u32;
        let high = (addr.0 >> 32) as u32;
        self.0[0].write(low);
        self.0[1].write(high);
    }
}
#[derive(Debug)]
#[repr(transparent)]
pub struct QueueReady(Volatile<u32, ReadWrite>);

impl QueueReady {
    pub fn ready(&self) {
        self.0.write(1);
    }

    pub fn unready(&self) {
        self.0.write(0);
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct Status(Volatile<u32, ReadWrite>);

impl Status {
    pub fn reset(&self) {
        self.0.write(0);
    }

    pub fn set_flag(&self, flag: StatusFlag) {
        self.0.write(self.0.read() | flag as u32);
        unsafe {
            core::arch::asm!("fence");
        }
    }

    pub fn failed(&self) -> bool {
        let bit = StatusFlag::Failed as u32;
        self.0.read() & bit == bit
    }

    pub fn needs_reset(&self) -> bool {
        let bit = StatusFlag::DeviceNeedsReset as u32;
        self.0.read() & bit == bit
    }

    pub fn is_set(&self, flag: StatusFlag) -> bool {
        self.0.read() & flag as u32 == flag as u32
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum StatusFlag {
    Acknowledge = 1,
    DeviceNeedsReset = 64,
    Driver = 2,
    DriverOk = 4,
    Failed = 128,
    FeaturesOk = 8,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct QueueNotify(Volatile<u32, Write>);

impl QueueNotify {
    pub fn notify(&self, queue: u32) {
        self.0.write(queue);
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct IntAck(Volatile<u32, Write>);

impl IntAck {
    pub fn ack(&self, mask: u32) {
        self.0.write(mask);
    }
}