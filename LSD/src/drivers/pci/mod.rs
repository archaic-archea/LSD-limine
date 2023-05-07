pub mod slot;
pub mod nvme;

use crate::println;

pub static PCI_HOST: crate::SetOnce<PCIHost> = crate::SetOnce::new(PCIHost::null());

/// # Safety
/// Only run ocne after the PCI_HOST variable is set
pub unsafe fn init() {
    if !PCI_HOST.set.load(core::sync::atomic::Ordering::Relaxed) {
        println!("PCI host not found");
        return;
    }

    let pci_host = PCI_HOST.get();

    println!("PCI host found {:#x?}", pci_host);

    for bus in 0..=255 {
        for dev in 0..=255 {
            let ecam_offset = pci_host.ecam_offset(bus, dev, 0);

            let new_slot = pci_host.ecam_region.start.add(ecam_offset) as *mut slot::Slot;

            let dev_id = (*new_slot).dev_id.read();

            if dev_id != 0xffff {
                println!("Device type: {:?}", (*new_slot).ident().dev_type());

                let bar_type = (*new_slot).bars.bar_kind(0);
                let bar_val = match bar_type {
                    slot::BarKind::Bits32 => (*new_slot).bars.read_u32(0) as u64,
                    slot::BarKind::Bits64 => (*new_slot).bars.read_u64(0),
                    _ => panic!()
                };
                
                let bar_virt = bar_val + crate::memory::HHDM_OFFSET.load(core::sync::atomic::Ordering::Relaxed);
                match (*new_slot).ident().dev_type() {
                    slot::DeviceType::Nvme => {
                        use crate::memory::{vmm, self};
                        vmm::map(
                            vmm::current_table().cast_mut(), 
                            memory::VirtualAddress(bar_virt), 
                            memory::PhysicalAddress(bar_val), 
                            vmm::PageLevel::from_usize(vmm::LEVELS.load(core::sync::atomic::Ordering::Relaxed) as usize), 
                            vmm::PageLevel::Level3, 
                            &mut crate::memory::pmm::REGION_LIST.lock(), 
                            vmm::PageFlags::READ | vmm::PageFlags::WRITE
                        );

                        let bar_virt = bar_virt as *mut nvme::controller_raw::RawController;

                        nvme::init(bar_virt);
                    },
                    dev => println!("Unprepared to handle device type {:?}", dev)
                }
            } else {
                break;
            }
        }
    }
}

#[derive(Debug)]
pub struct PCIHost {
    pub interrupt_map_mask: usize,
    pub interrupt_map: usize,
    pub ecam_region: EcamRegion,
    pub bus_range: core::ops::RangeInclusive<u8>,
}

impl PCIHost {
    pub const fn null() -> Self {
        Self { 
            interrupt_map_mask: 0, 
            interrupt_map: 0, 
            ecam_region: EcamRegion { 
                start: core::ptr::null_mut(), 
                size: 0
            }, 
            bus_range: 0..=0, 
        }
    }

    pub fn ecam_offset(&self, bus: u8, dev: u8, func: u8) -> usize {
        (((bus - self.bus_range.start()) as usize) << 20)
        | ((dev as usize) << 15)
        | ((func as usize) << 12)
    }
}

#[derive(Debug)]
pub struct EcamRegion {
    pub start: *mut u8,
    pub size: usize,
}

impl EcamRegion {}