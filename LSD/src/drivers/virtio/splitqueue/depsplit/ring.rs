pub struct Available {
    pub queue: &'static mut crate::memory::DmaRegion<AvailableRing>
}

impl Available {
    pub fn push(&mut self, index: super::SplitqueueIndex<super::descriptor::SplitDescriptor>) {
        let queue_ptr = self.queue.vi().as_ptr();

        unsafe {
            let queue_index = (*queue_ptr).index;
            let ring_index = (*queue_ptr).index % (*queue_ptr).ring.len() as u16;
            // This is likely overkill, but better to be safe than sorry!
            core::ptr::write_volatile(&mut (*queue_ptr).ring[ring_index as usize], index.0);

            // From the VirtIO spec:
            // > 2.7.13.3.1 Driver Requirements: Updating idx
            // >
            // > The driver MUST perform a suitable memory barrier before the idx
            // > update, to ensure the device sees the most up-to-date copy.
            core::arch::asm!("fence");

            core::ptr::write_volatile(&mut (*queue_ptr).index, queue_index.wrapping_add(1));
        }
    }
}

#[repr(C)]
pub struct AvailableRing {
    flags: RingFlags,
    index: u16,
    ring: [u16],
}

bitflags::bitflags! {
    pub struct RingFlags: u16 {
        const NO_INT = 0b1;
    }
}

#[repr(C)]
pub struct UsedQueue {
    pub queue: &'static mut crate::memory::DmaRegion<VirtqueueUsed>,
    pub last_seen: u16,
}

impl UsedQueue {
    pub fn pop(&mut self) -> Option<VirtqueueUsedElement> {
        let queue_ptr = self.queue.virt().as_ptr();

        let index = unsafe { core::ptr::read_volatile(&(*queue_ptr).index) };
        match self.last_seen == index {
            // No new used elements
            true => None,
            false => {
                let used = unsafe {
                    core::ptr::read_volatile(&(*queue_ptr).ring[self.last_seen as usize % (*queue_ptr).ring.len()])
                };
                self.last_seen = self.last_seen.wrapping_add(1);

                Some(used)
            }
        }
    }
}

#[repr(C)]
pub struct VirtqueueUsed {
    flags: u16,
    index: u16,
    ring: [VirtqueueUsedElement],
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct VirtqueueUsedElement {
    pub start_index: u32,
    pub length: u32,
}