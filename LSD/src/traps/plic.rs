use core::sync::atomic::{AtomicPtr, Ordering};

use spin::Mutex;

use crate::current_context;

pub static PLIC_ADDR: AtomicPtr<sifive_plic::Plic> = AtomicPtr::new(core::ptr::null_mut());
pub static INT_HANDLERS: Mutex<[fn (usize); 64]> = Mutex::new([unknown; 64]);

pub fn handle_external() {
    let addr = PLIC_ADDR.load(Ordering::Relaxed);
    let context = current_context();

    unsafe {
        let claim = (*addr).claim(context).expect("No claim available");
        
        INT_HANDLERS.lock()[claim.interrupt_id()](claim.interrupt_id());

        claim.complete();
    }
}

fn unknown(id: usize) {
    panic!("Unknown external interrupt 0x{:x}", id);
}