pub use crate::alloc_core::alloc::*;

#[global_allocator]
static ALLOCATOR: Alloc = Alloc(vmem::Vmem::new(crate::borrow::Cow::Borrowed("HEAP"), 1, None));

pub struct Alloc(vmem::Vmem<'static, 'static>);

unsafe impl crate::alloc_core::alloc::GlobalAlloc for Alloc {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let vmem_layout = vmem::Layout::new(layout.size());
        let vmem_layout = vmem_layout.align(layout.align());
        self.0.alloc_constrained(vmem_layout, vmem::AllocStrategy::BestFit).unwrap_or_else(|_err| self.handle_alloc_err(vmem_layout)) as *mut u8
    }

    unsafe fn alloc_zeroed(&self, layout: core::alloc::Layout) -> *mut u8 {
        let location = self.alloc(layout);

        for i in 0..layout.size() {
            *location.add(i) = 0;
        }

        location
    }
    
    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        self.0.free_constrained(ptr as usize, layout.size());
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: core::alloc::Layout, new_size: usize) -> *mut u8 {
        todo!("Reallocate");
    }
}

impl Alloc {
    fn handle_alloc_err(&self, layout: vmem::Layout) -> usize {
        let new_space = crate::raw_calls::extend_heap(0x1000);
        self.0.add(new_space as usize, 0x1000).unwrap();
        self.0.alloc_constrained(layout, vmem::AllocStrategy::BestFit).unwrap_or_else(|_| self.handle_alloc_err(layout))
    }
}