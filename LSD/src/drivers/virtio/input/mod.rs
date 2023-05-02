use core::sync::atomic::AtomicPtr;

use crate::{println, print};

use super::splitqueue;

pub mod structs;

pub static INPUT_DEV: AtomicPtr<Input> = AtomicPtr::new(core::ptr::null_mut());

/// # Safety
/// Only called once per input device
pub unsafe fn init(device_ptr: *mut super::VirtIOHeader) {
    let mut device = Input::new(device_ptr, 8);
    let virt = device.eventqueue.descriptors.queue.virt().as_ptr();

    let index = splitqueue::SplitqueueIndex::new(0);

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
        use splitqueue::descriptor;
        
        let flag = if i != 7 {
            descriptor::DescriptorFlags::NEXT |
            descriptor::DescriptorFlags::WRITE
        } else {
            descriptor::DescriptorFlags::WRITE
        };

        let section = crate::memory::DmaRegion::<structs::InputEvent>::new_raw(
            (), 
            true
        ).leak();

        (*virt)[i] = descriptor::SplitDescriptor {
            address: section.phys().0,
            length: core::mem::size_of::<structs::InputEvent>() as u32,
            flags: flag,
            next: (i + 1) as u16,
        };
    }
}

pub struct Input {
    pub header: *mut super::VirtIOHeader,
    pub eventqueue: super::splitqueue::SplitQueue,
    pub statusqueue: super::splitqueue::SplitQueue,
}

impl Input {
    /// # Safety
    /// Only call once per virtio device
    pub unsafe fn new(header: *mut super::VirtIOHeader, queue_size: usize) -> Self {
        use super::StatusFlag;

        let event = super::splitqueue::SplitQueue::new(queue_size).unwrap();
        let status = super::splitqueue::SplitQueue::new(queue_size).unwrap();

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
        (*new_self.header).queue_desc.set(new_self.eventqueue.descriptors.queue.phys());
        (*new_self.header).queue_avail.set(new_self.eventqueue.available.queue.phys());
        (*new_self.header).queue_used.set(new_self.eventqueue.used.queue.phys());
        (*new_self.header).queue_ready.ready();


        (*new_self.header).queue_sel.write(1);
        (*new_self.header).queue_desc.set(new_self.statusqueue.descriptors.queue.phys());
        (*new_self.header).queue_avail.set(new_self.statusqueue.available.queue.phys());
        (*new_self.header).queue_used.set(new_self.statusqueue.used.queue.phys());
        (*new_self.header).queue_ready.ready();

        (*new_self.header).status.set_flag(StatusFlag::DriverOk);

        if (*new_self.header).status.failed() {
            println!("Input Error during queue setup");
        }

        new_self
    }

    pub fn command(&mut self, cmd_select: structs::InputConfigSelect, subsel: u8) {
        let command = structs::InputConfig {
            select: cmd_select,
            subsel,
            size: 0,
            _reserved: [0; 5],
            union: structs::InputConfigUnion {
                bitmap: [0; 128]
            }
        };

        unsafe {
            let dma_region = crate::memory::DmaRegion::new_raw((), true).leak();
            *dma_region.virt().as_ptr() = command;

            let descriptor = super::splitqueue::descriptor::SplitDescriptor {
                address: dma_region.phys().0,
                length: core::mem::size_of::<structs::InputConfig>() as u32,
                flags: super::splitqueue::descriptor::DescriptorFlags::WRITE,
                next: 0,
            };

            (*self.statusqueue.descriptors.queue.virt().as_ptr())[0] = descriptor;

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

        let desc = input.statusqueue.descriptors.queue.virt().as_mut();
        let desc = &desc[0];

        let base = desc.address + crate::memory::HHDM_OFFSET.load(core::sync::atomic::Ordering::Relaxed);
        let ptr = base as *mut structs::InputConfig;

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