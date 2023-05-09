#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum InputConfigSelect {
    Unset = 0x00,
    IDName = 0x01,
    IDSerial = 0x02,
    IDDevIDs = 0x03,
    PropBits = 0x10,
    EVBits = 0x11,
    ABSInfo = 0x12,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct InputABSInfo {
    pub min: u32,
    pub max: u32,
    pub fuzzd: u32,
    pub flat: u32,
    pub res: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct InputDevIDs {
    pub bustype: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct InputConfig {
    pub select: InputConfigSelect,
    pub subsel: u8,
    pub size: u8,
    pub _reserved: [u8; 5],
    pub union: InputConfigUnion
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union InputConfigUnion {
    pub string: [u8; 128],
    pub bitmap: [u8; 128],
    pub abs: InputABSInfo,
    pub ids: InputDevIDs,
}

#[repr(C)]
pub struct InputEvent {
    pub event_type: u16,
    pub code: u16,
    pub val: u32
}