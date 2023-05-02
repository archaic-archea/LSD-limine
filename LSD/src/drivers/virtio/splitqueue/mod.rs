/// Splitqueue code is heavily based off repnop's code in vanadinite, which is licensed under MPL-2.0
/// For more info visit the repository `https://github.com/repnop/vanadinite`

use alloc::collections::VecDeque;

use self::descriptor::SplitDescriptorTable;

pub mod descriptor;
pub mod ring;

#[repr(C)]
pub struct SplitQueue {
    pub queue_size: usize,
    pub freelist: VecDeque<u16>,
    pub descriptors: descriptor::SplitDescriptorTable,
    pub available: ring::Available,
    pub used: ring::UsedQueue,
}

impl SplitQueue {
    pub fn new(queue_size: usize) -> Result<Self, SplitVirtqueueError> {
        match queue_size {
            n if !n.is_power_of_two() => return Err(SplitVirtqueueError::NotPowerOfTwo),
            0..=32768 => {}
            _ => return Err(SplitVirtqueueError::TooLarge),
        }
        let freelist = (0..queue_size as u16).collect();

        let descriptors = SplitDescriptorTable {
            queue: unsafe {
                crate::memory::DmaRegion::zeroed_many(queue_size).assume_init().leak()
            }
        };
        let available = ring::Available {
            queue: unsafe {
                crate::memory::DmaRegion::new_raw(queue_size, true).leak()
            }
        };
        let used = ring::UsedQueue {
            queue: unsafe {
                crate::memory::DmaRegion::new_raw(queue_size, true).leak()
            },
            last_seen: 0,
        };

        Ok(Self { 
            queue_size, 
            freelist, 
            descriptors, 
            available, 
            used 
        })
    }
}

#[repr(transparent)]
pub struct SplitqueueIndex<T>(u16, core::marker::PhantomData<T>);

impl<T> SplitqueueIndex<T> {
    pub fn new(index: u16) -> Self {
        Self(index, core::marker::PhantomData)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SplitVirtqueueError {
    MemoryAllocationError,
    NotPowerOfTwo,
    TooLarge,
}