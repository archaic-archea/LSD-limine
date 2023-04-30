use core::sync::atomic::AtomicPtr;

use alloc::vec::Vec;
use spin::Mutex;

use crate::volatile::*;

pub mod splitqueue;

pub static VIRTIO_LIST: Mutex<Vec<AtomicPtr<VirtIOHeader>>> = Mutex::new(Vec::new());

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
    pub queue_num_max: Volatile<u32, Read>, // 0x34
    pub queue_num: Volatile<u32, Write>, // 0x38
    _reserved3: [u32; 2], // 0x3C
    pub queue_ready: Volatile<u32, ReadWrite>, // 0x44
    _reserved4: [u32; 2], // 0x48
    pub queue_notify: Volatile<u32, Write>, // 0x50

    _reserved5: [u32; 3], // 0x54

    pub int_status: Volatile<u32, Read>, // 0x60
    pub int_ack: Volatile<u32, Write>, // 0x64

    _reserved6: [u32; 2], // 0x68

    pub status: Volatile<u32, ReadWrite>, // 0x70

    _reserved7: [u32; 3], // 0x74

    pub queue_descriptor_low: Volatile<u32, Write>, // 0x80
    pub queue_descriptor_high: Volatile<u32, Write>, // 0x84

    _reserved8: [u32; 2], // 0x88

    pub queue_driver_low: Volatile<u32, Write>, // 0x90
    pub queue_driver_high: Volatile<u32, Write>, // 0x94

    _reserved9: [u32; 2], // 0x98

    pub queue_dev_low: Volatile<u32, Write>, // 0xA0
    pub queue_dev_high: Volatile<u32, Write>, // 0xA4

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