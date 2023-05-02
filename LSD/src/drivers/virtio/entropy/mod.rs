use crate::println;

pub struct Entropy {
    header: *mut super::VirtIOHeader,
    req: super::splitqueue::SplitQueue,
}

impl Entropy {
    /// # Safety
    /// Only call once per device
    pub unsafe fn init(head: *mut super::VirtIOHeader) -> &'static mut Entropy {
        let queue = super::splitqueue::SplitQueue::new(1).unwrap();

        let new_self = Self { 
            header: head, 
            req: queue
        };

        (*new_self.header).status.reset();
        (*new_self.header).status.set_flag(super::StatusFlag::Acknowledge);
        (*new_self.header).status.set_flag(super::StatusFlag::Driver);

        (*new_self.header).dev_feat_sel.write(0);
        (*new_self.header).driver_feat_sel.write(0);
        (*new_self.header).driver_feat.write(0);

        (*new_self.header).status.set_flag(super::StatusFlag::FeaturesOk);
        if !(*new_self.header).status.is_set(super::StatusFlag::FeaturesOk) {
            println!("Features not accepted");
        }

        (*new_self.header).queue_sel.write(0);
        (*new_self.header).queue_avail.set(new_self.req.available.queue.phys());
        (*new_self.header).queue_desc.set(new_self.req.descriptors.queue.phys());
        (*new_self.header).queue_used.set(new_self.req.used.queue.phys());
        (*new_self.header).queue_ready.ready();

        (*new_self.header).status.set_flag(super::StatusFlag::DriverOk);

        let boxed = alloc::boxed::Box::new(new_self);
        let dev_ref = alloc::boxed::Box::leak(boxed);

        dev_ref
    }

    pub fn request(&mut self, byte_len: usize) {
        println!("Requesting random number");
        let dma: &'static mut crate::memory::DmaRegion<[u8]> = unsafe {crate::memory::DmaRegion::new_many(byte_len).assume_init().leak()};

        let desc = super::splitqueue::descriptor::SplitDescriptor {
            address: dma.phys().0,
            length: byte_len as u32,
            flags: super::splitqueue::descriptor::DescriptorFlags::WRITE,
            next: 0,
        };

        println!("{:#x?}", desc);

        unsafe {
            for entry in (*dma.virt().as_ptr()).iter_mut() {
                *entry = 0; 
            }

            (*self.req.descriptors.queue.virt().as_ptr())[0] = desc;

            let index = super::splitqueue::SplitqueueIndex::new(0);
            self.req.available.push(index);

            (*self.header).queue_notify.notify(0);
        }

        //&'static mut 
    }
}