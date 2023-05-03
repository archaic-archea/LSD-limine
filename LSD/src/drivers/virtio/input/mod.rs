use core::sync::atomic::AtomicPtr;

use crate::{println, print};

pub mod structs;

pub static INPUT_DEV: AtomicPtr<Input> = AtomicPtr::new(core::ptr::null_mut());

/// # Safety
/// Only called once per input device
pub unsafe fn init(device_ptr: *mut super::VirtIOHeader) {
    let mut device = Input::new(device_ptr, 8);
    let index = device.eventqueue.alloc_descriptor().unwrap();

    device.eventqueue.available.push(index);

    let boxed = alloc::boxed::Box::new(device);
    let dev_ref = alloc::boxed::Box::leak(boxed);

    INPUT_DEV.store(dev_ref, core::sync::atomic::Ordering::Relaxed);

    println!("Name command sending");
    dev_ref.command(structs::InputConfigSelect::IDName, 0);

    println!("Serial command sending");
    dev_ref.command(structs::InputConfigSelect::IDSerial, 0);

    println!("Dev IDs command sending");
    dev_ref.command(structs::InputConfigSelect::IDDevIDs, 0);

    println!("EVBits command sending");
    dev_ref.command(structs::InputConfigSelect::EVBits, 0);

    for i in 0..8 {
    
        let flag = if i != 7 {
            super::splitqueue::DescriptorFlags::NEXT |
            super::splitqueue::DescriptorFlags::WRITE
        } else {
            super::splitqueue::DescriptorFlags::WRITE
        };

        let section = crate::memory::DmaRegion::<structs::InputEvent>::new_raw(
            (), 
            true
        );

        dev_ref.eventqueue.descriptors.write(index, super::splitqueue::VirtqueueDescriptor {
            address: section.physical_address(),
            length: core::mem::size_of::<structs::InputEvent>() as u32,
            flags: flag,
            next: super::splitqueue::SplitqueueIndex::new(i + 1),
        });
    }
}

pub struct Input {
    pub header: *mut super::VirtIOHeader,
    pub eventqueue: super::splitqueue::SplitVirtqueue,
    pub statusqueue: super::splitqueue::SplitVirtqueue,
}

impl Input {
    /// # Safety
    /// Only call once per virtio device
    pub unsafe fn new(header: *mut super::VirtIOHeader, queue_size: usize) -> Self {
        use super::StatusFlag;

        let event = super::splitqueue::SplitVirtqueue::new(queue_size).unwrap();
        let status = super::splitqueue::SplitVirtqueue::new(queue_size).unwrap();

        let new_self = Self {
            header,
            eventqueue: event,
            statusqueue: status,
        };

        (*new_self.header).status.reset();
        (*new_self.header).status.set_flag(StatusFlag::Acknowledge);
        (*new_self.header).status.set_flag(StatusFlag::Driver);

        //Negotiate features
        (*new_self.header).driver_feat_sel.write(0);
        (*new_self.header).dev_feat_sel.write(0);
        (*new_self.header).driver_feat.write(0);

        //check that device likes the features
        (*new_self.header).status.set_flag(StatusFlag::FeaturesOk);
        if !(*new_self.header).status.is_set(StatusFlag::FeaturesOk) {
            println!("Features not accepted");
        }

        (*new_self.header).queue_sel.write(0);
        (*new_self.header).queue_desc.set(new_self.eventqueue.descriptors.physical_address());
        (*new_self.header).queue_avail.set(new_self.eventqueue.available.physical_address());
        (*new_self.header).queue_used.set(new_self.eventqueue.used.physical_address());
        (*new_self.header).queue_size.write(new_self.eventqueue.queue_size());
        (*new_self.header).queue_ready.ready();


        (*new_self.header).queue_sel.write(1);
        (*new_self.header).queue_desc.set(new_self.statusqueue.descriptors.physical_address());
        (*new_self.header).queue_avail.set(new_self.statusqueue.available.physical_address());
        (*new_self.header).queue_used.set(new_self.statusqueue.used.physical_address());
        (*new_self.header).queue_size.write(new_self.statusqueue.queue_size());
        (*new_self.header).queue_ready.ready();

        (*new_self.header).status.set_flag(StatusFlag::DriverOk);

        if (*new_self.header).status.failed() {
            println!("Input Error during queue setup");
        }

        new_self
    }

    pub fn command(&mut self, select: structs::InputConfigSelect, subsel: u8) {
        let command = structs::InputConfig {
            select,
            subsel,
            size: 0,
            _reserved: [0; 5],
            union: structs::InputConfigUnion {
                bitmap: [0; 128]
            }
        };

        unsafe {
            let mut dma_region: crate::memory::DmaRegion<structs::InputConfig> = crate::memory::DmaRegion::new_raw((), true);
            *dma_region = command;

            let descriptor = super::splitqueue::VirtqueueDescriptor {
                address: dma_region.physical_address(),
                length: core::mem::size_of::<structs::InputConfig>() as u32,
                flags: super::splitqueue::DescriptorFlags::WRITE,
                next: super::splitqueue::SplitqueueIndex::new(0),
            };

            let idx = self.statusqueue.alloc_descriptor().unwrap();
            self.statusqueue.descriptors.write(idx, descriptor);

            self.statusqueue.available.push(super::splitqueue::SplitqueueIndex::new(0));
            (*self.header).queue_notify.notify(1);
        }
    }
}

pub fn handle_int(_id: usize) {
    use structs::InputConfigSelect;

    let input = INPUT_DEV.load(core::sync::atomic::Ordering::Relaxed);
    let input = unsafe {&mut *input};

    unsafe {
        println!("Input int");
        let status = (*input.header).int_status.read();

        let index = input.statusqueue.used.pop();

        let desc = match index {
            Some(index) => {
                let index = super::splitqueue::SplitqueueIndex::new(index.start_index as u16);

                input.statusqueue.descriptors.read(index)
            },
            None => {
                let index = input.eventqueue.used.pop().unwrap_or_else(|| {panic!("No used entry")});
                let index = super::splitqueue::SplitqueueIndex::new(index.start_index as u16);

                input.eventqueue.descriptors.read(index)
            }
        };

        let base = desc.address.0 + crate::memory::HHDM_OFFSET.load(core::sync::atomic::Ordering::Relaxed);
        let ptr = base as *mut structs::InputConfig;

        println!("Handling input int on config {:?}", (*ptr).select);

        match (*ptr).select {
            InputConfigSelect::IDName => {
                if (*ptr).size != 0 {
                    println!("Reading name");
        
                    for index in 0..(*ptr).size {
                        print!("{}", (*ptr).union.string[index as usize] as char);
                    }
        
                    println!();
                } else {
                    println!("No data sent for name");
                }
            },
            req => {panic!("Cant handle request `{:?}` yet", req)}
        }

        (*input.header).int_ack.ack(status);
    }
}