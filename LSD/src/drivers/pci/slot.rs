use crate::volatile::{Volatile, Read, ReadWrite};

#[derive(Debug)]
#[repr(C)]
pub struct Slot {
    pub vendor_id: Volatile<u16, Read>,
    pub dev_id: Volatile<u16, Read>,
    pub command: Volatile<u16, Read>,
    status: Volatile<u16, Read>,
    rev_id: Volatile<u8, Read>,
    pub class_code: ClassCode,
    cacheline_size: Volatile<u8, Read>,
    lat_timer: Volatile<u8, Read>,
    pub head_type: HeadType,
    bist: Volatile<u8, Read>,
    pub bars: Bars,
    cardbus_cis_ptr: Volatile<u32, Read>,
    subsys_vend_id: Volatile<u16, Read>,
    subsys_id: Volatile<u16, Read>,
    exp_rom_base: Volatile<u32, Read>,
    cap_ptr: Volatile<u8, Read>,
    _res: [u8; 7],
    int_lin: Volatile<u8, Read>,
    int_pin: Volatile<u8, Read>,
    min_gnt: Volatile<u8, Read>,
    max_lat: Volatile<u8, Read>
}

impl Slot {
    pub fn ident(&self) -> DeviceIdent {
        // SAFETY: Register 0 and 2 are always implemented.
        let class_reg = unsafe {self.read_reg(2)};

        let vendor_id = self.vendor_id.read();
        let device_id = self.dev_id.read();
        
        let revision = class_reg as u8;
        let prog_if = (class_reg >> 8) as u8;
        let subclass = (class_reg >> 16) as u8;
        let class = (class_reg >> 24) as u8;

        DeviceIdent {
            vendor_id,
            device_id,
            class,
            subclass,
            prog_if,
            revision
        }
    }

    pub unsafe fn read_reg(&self, index: usize) -> u32 {
        assert!(index < 1024);
        let ecam: *const Self = self;
        let ecam = ecam as *mut u32;

        ecam.add(index).read_volatile()
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct ClassCode {
    high: Volatile<u8, Read>,
    mid: Volatile<u8, Read>,
    low: Volatile<u8, Read>
}

impl ClassCode {
    pub fn read(&self) -> u32 {
        let high = self.high.read() as u32;
        let mid = self.mid.read() as u32;
        let low = self.low.read() as u32;

        (high << 16) | (mid << 8) | low
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct HeadType(Volatile<u8, Read>);

impl HeadType {
    pub fn is_multifunction(&self) -> bool {
        let val = self.0.read() >> 7;

        val == 1
    }

    pub fn head_type(&self) -> u8 {
        self.0.read() & 0x7f
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct Bars([Volatile<u32, ReadWrite>; 6]);

impl Bars {
    pub fn read_u32(&self, index: usize) -> u32 {
        self.0[index].read() & !0b111
    }

    pub fn read_u64(&self, index: usize) -> u64 {
        let high = self.0[index].read() as u64;
        let low = self.0[index + 1].read() as u64;
        let low = low & !0b111;

        (high << 32) | low
    }

    pub fn bar_kind(&self, index: usize) -> BarKind {
        let bar = self.0[index].read();
        
        if bar & 1 == 1 {
            BarKind::IOSpace
        } else {
            match bar >> 1 & 0x3 {
                0x0 => BarKind::Bits32,
                0x2 => BarKind::Bits64,
                _ => panic!(),
            }
        }
    }
}

#[derive(Debug)]
pub enum BarKind {
    IOSpace,
    Bits64,
    Bits32,
}

pub struct DeviceIdent {
    pub vendor_id: u16,
    pub device_id: u16,
    pub class: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub revision: u8,
}

impl DeviceIdent {
    pub fn dev_type(&self) -> DeviceType {
        let mut s = DeviceType::Unknown;
    
        #[allow(clippy::single_match)]
        match self.class {
            0x1 => match self.subclass {
                0x0 => s = DeviceType::SCSIBusController,
                0x6 => match self.prog_if {
                    0x1 => s = DeviceType::Ahci1,
                    _ => {}
                },
                0x8 => match self.prog_if {
                    0x2 => s = DeviceType::Nvme,
                    _ => {}
                },
                _ => {}
            },
            0x6 => match self.subclass {
                0x0 => s = DeviceType::HostBridgeController,
                _ => {}
            },
            _ => {}
        }
    
        s
    }
}

#[derive(Debug)]
pub enum DeviceType {
    Unknown,
    SCSIBusController,
    Nvme,
    Ahci1,
    HostBridgeController
}