bitfield::bitfield! {
    pub struct Sstatus(u64);
    _res0, _: 0;
    _res1, _: 4, 2;
    _res2, _: 7;
    _res3, _: 12, 11;
    _res4, _: 17;
    _res5, _: 31, 20;
    _res6, _: 62, 34;

    /// When true, interrupts are enabled
    pub sie, set_sie: 1; 

    /// Handled by machine
    spie, set_spie: 5;

    /// When true, system is big endian
    pub ube, set_ube: 6;
    
    /// When false, an sret will jump to user-mode
    pub spp, set_spp: 8;
    
    /// Unknown
    vs, set_vs: 10, 9;
    
    /// Unknown
    fs, set_fs: 14, 13;
    
    /// Unknown
    xs, set_xs: 16, 15;
    
    /// When true, supervisor has access to user memory
    pub sum, set_sum: 18;
    
    /// When true, makes all executable pages readable
    pub mxr, set_mxr: 19;
    
    /// Controls user-mode XLEN
    uxl, set_uxl: 33, 32;
    
    /// Unknown
    sd, set_sd: 63;
}

impl Sstatus {
    pub fn new() -> Self {
        unsafe {
            let mut sstatus_val: u64;

            core::arch::asm!("csrr {sval}, sstatus", sval = out(reg) sstatus_val);

            return core::mem::transmute(sstatus_val);
        }
    }

    pub unsafe fn set(&self) {
        core::arch::asm!("csrw sstatus, {sval}", sval = in(reg) self.0);
    }
}

bitfield::bitfield! {
    pub struct Sie(u16);

    zero0, set_zero0: 0;
    zero1, set_zero1: 4, 2;
    zero2, set_zero2: 8, 6;
    zero3, set_zero3: 15, 10;
    
    /// When true supervisor software interrupts are enabled
    pub ssie, set_ssie: 1;

    /// When true supervisor timer interrupts are enabled
    pub stie, set_stie: 5;

    /// When true supervisor external interrupts are enabled
    pub seie, set_seie: 9;
}

impl Sie {
    pub fn new() -> Self {
        unsafe {
            let mut sie_val: u16;

            core::arch::asm!("csrr {sval}, sie", sval = out(reg) sie_val);

            return core::mem::transmute(sie_val);
        }
    }

    pub unsafe fn set(&self) {
        core::arch::asm!("csrw sie, {sval}", sval = in(reg) self.0);
    }
}

pub struct Time;

impl Time {
    pub fn get() -> u64 {
        unsafe {
            let ret: u64;

            core::arch::asm!("csrr {ret}, time", ret = out(reg) ret);

            return ret;
        }
    }
}