use crate::volatile::{
    Volatile, 
    Read, 
    ReadWrite
};

#[repr(C)]
pub struct RawController {
    /// Controller Capabilities
    pub cap: Volatile<u64, Read>,       // 0x00

    /// Version
    pub vs: Version,                    // 0x08

    /// Interrupt Mask Set
    pub intms: Volatile<u32, ReadWrite>,    // 0x0C

    /// Interrupt Mask Clear
    pub intmc: Volatile<u32, ReadWrite>,    // 0x10

    /// Controller Configuration
    pub cc: Volatile<u32, ReadWrite>,            // 0x14
    _reserved1: u32,                    // 0x18

    /// Controller Status
    pub csts: Volatile<u32, Read>,          // 0x1C

    /// NVM Subsystem Reset
    pub nssr: Volatile<u32, Read>,          // 0x20

    /// Admin Queue Attributes
    pub aqa: Volatile<u32, ReadWrite>,           // 0x24

    /// Admin Submission Queue Base Address
    pub asq: Volatile<u64, ReadWrite>,      // 0x28

    /// Admin Completion Queue Base Address
    pub acq: Volatile<u64, ReadWrite>,      // 0x30

    /// Controller Memory Buffer Location
    pub cmbloc: Volatile<u32, ReadWrite>,   // 0x38

    /// Controller Memory Buffer Size
    pub cmbsz: Volatile<u32, ReadWrite>,    // 0x3C

    /// Boot Partition Info
    pub bpinfo: Volatile<u32, ReadWrite>,   // 0x40

    /// Boot Partition Read Select
    pub bprsel: Volatile<u32, ReadWrite>,   // 0x44

    /// Boot Partition Memory Buffer Location
    pub bpmbl: Volatile<u64, ReadWrite>,    // 0x48

    /// Controller Memory Buffer Memory Space Control
    pub cmbmsc: Volatile<u64, ReadWrite>,   // 0x50

    /// Controller Memory Buffer Status
    pub cmbsts: Volatile<u32, ReadWrite>,   // 0x58

    /// Controller Memory Buffer Elasticity Buffer Size
    pub cmbebs: Volatile<u32, ReadWrite>,   // 0x5C

    /// Controller Memory Buffer Sustained Write Throughput
    pub cmbswtp: Volatile<u32, ReadWrite>,  // 0x60

    /// NVM Subsystem Shutdown
    pub nssd: Volatile<u32, ReadWrite>,     // 0x64

    /// Controller Ready Timeouts
    pub crto: Volatile<u32, ReadWrite>,     // 0x68
    _reserved3: [u8; 3476],             // 0x6C

    /// Persistent Memory Capabilities
    pub pmrcap: Volatile<u32, ReadWrite>,   // 0xE00

    /// Persistent Memory Region Control
    pub pmrctl: Volatile<u32, ReadWrite>,   // 0xE04

    /// Persistent Memory Region Status
    pub pmrsts: Volatile<u32, ReadWrite>,   // 0xE08
    
    /// Persistent Memory Region Elasticity Buffer Size
    pub pmrebs: Volatile<u32, ReadWrite>,   // 0xE0C

    /// Persistent Memory Region Sustained Write Throughput
    pub pmrswtp: Volatile<u32, ReadWrite>,  // 0xE10

    /// Persistent Memory Region Controller Memory Space Control Lower
    pub pmrmscl: Volatile<u32, ReadWrite>,  // 0xE14

    /// Persistent Memory Region Controller Memory Space Control Upper
    pub pmrmscu: Volatile<u32, ReadWrite>,  // 0xE18
    _reserved4: [u8; 484]               // 0xE1C
}

#[repr(transparent)]
pub struct Version(Volatile<u32, Read>);

impl Version {
    pub fn version(&self) -> [u8; 3] {
        let mut version = [0; 3];

        let read = self.0.read();

        version[0] = ((read >> 16) & 0xff) as u8;
        version[1] = ((read >> 8) & 0xff) as u8;
        version[2] = (read & 0xff) as u8;

        version
    }

    pub fn version_str(&self) -> alloc::string::String {
        let mut version_string = alloc::string::String::new();
        let version = self.version();

        version_string.push_str(&alloc::format!("{}.", version[0]));
        version_string.push_str(&alloc::format!("{}.", version[1]));
        version_string.push_str(&alloc::format!("{}", version[2]));

        version_string
    }
}