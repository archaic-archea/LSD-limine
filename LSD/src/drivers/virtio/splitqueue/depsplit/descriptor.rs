pub struct SplitDescriptorTable {
    pub queue: &'static mut crate::memory::DmaRegion<[SplitDescriptor]>
}

#[derive(Debug)]
#[repr(C)]
pub struct SplitDescriptor {
    /// Little Endian physical address
    pub address: u64,

    /// Little Endian
    pub length: u32,

    /// Little Endian descriptor flags
    pub flags: DescriptorFlags,

    /// Little Endian index of the next descriptor if the `NEXT` flag is true
    pub next: u16,
}

bitflags::bitflags! {
    #[derive(Debug)]
    pub struct DescriptorFlags: u16 {
        const NEXT =        0b001;
        const WRITE =       0b010;
        const INDIRECT =    0b100;
    }
}