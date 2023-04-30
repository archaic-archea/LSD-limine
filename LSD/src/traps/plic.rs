use core::sync::atomic::{AtomicPtr, Ordering};

use crate::current_context;

pub static PLIC_ADDR: AtomicPtr<sifive_plic::Plic> = AtomicPtr::new(core::ptr::null_mut());

pub fn handle_external() {
    let addr = PLIC_ADDR.load(Ordering::Relaxed);
    let context = current_context();

    unsafe {
        let claim = (*addr).claim(context).expect("No claim available");
        
        match claim.interrupt_id() {
            id => {
                panic!("Unknown interrupt 0x{:x}", id);
            }
        }

        claim.complete();
    }
}