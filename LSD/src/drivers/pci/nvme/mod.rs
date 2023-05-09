pub mod queue;
pub mod controller_raw;

use core::time::Duration;

use libsa::endian::u32_le;

use controller_raw::RawController;

use crate::{
    println, 
    size_of,
    memory::vmm::PAGE_SHIFT,
    volatile::Volatile
};

/// An NVM Express Host Controller
pub struct Controller {
    caps: Capabilities,
    reg_base: *mut u32_le,
    doorbell_base: *mut Volatile<u32_le>,
    admin_queue: QueuePair,
    io_queue: QueuePair,
}

impl Controller {
    pub fn status(&self) -> Status {
        let bits = unsafe { self.reg_base.byte_add(0x1c).read_volatile().get() };
        Status::new(bits)
    }

    fn wait_ready(&mut self) -> Option<()> {
        let timeout = crate::timing::Timeout::start(self.caps.timeout());

        loop {
            let status = self.status();

            if status.contains(Status::RDY) {
                return Some(());
            }

            if status.contains(Status::CFS) {
                return None;
            }

            if timeout.expired() {
                return None;
            }
        }
    }

    fn admin_cmd(&mut self) {
    }
}

/// # Safety
/// Only call once per controller
pub unsafe fn init(controller: *mut RawController) -> Controller {
    let caps = Capabilities::new((*controller).cap.read());

    println!(
        "NVMe Controller found\nCap {:?}\nVersion {}", 
        caps.timeout(), 
        (*controller).vs.version_str()
    );

    println!("Max page size 0x{:x}", caps.max_page_size());
    println!("Min page size 0x{:x}", caps.min_page_size());

    assert!(caps.min_page_size() <= crate::memory::vmm::PageSize::Small as usize, "Smallest page size is too small for controller");
    assert!(caps.max_page_size() >= crate::memory::vmm::PageSize::Small as usize, "Smallest page size is too big for controller");
    assert!(caps.contains(Capabilities::NVM_COMMAND_SET), "No command set available");

    // Disable so we can reconfigure it
    (*controller).cc.write(0);

    // Options set with value 0:
    //  - Controller Ready Independent of Media: no
    //      CSTS.RDY will not be set until the connected devices are also ready.
    //  - Shutdown Notification: None
    //  - Arbitration Mechanism: Round Robin
    //      Anything else may not be supported and offers no benefit to a single queue.
    //  - I/O Command Set Selected: NVM Command Set
    let mut conf = 0u32;

    // Set Memory Page and I/O Queue Entry sizes.
    conf |= (PAGE_SHIFT - 12) << 7;
    conf |= size_of!(SubmissionQueueEntry).ilog2() << 16;
    conf |= size_of!(CompletionQueueEntry).ilog2() << 20;

    // Allocate the queues
    let admin_queue = QueuePair::new();
    let io_queue = QueuePair::new();

    (*controller).aqa.write((admin_queue.len() - 1) as u32 * 0x00010001);
    (*controller).asq.write(admin_queue.subq.addr() as u64);
    (*controller).acq.write(admin_queue.comq.addr() as u64);

    let mut ctlr = Controller {
        caps,
        reg_base: controller.cast(),
        doorbell_base: unsafe { controller.byte_add(0x1000).cast() },
        admin_queue,
        io_queue,
    };

    // Enable the controller and wait for it to be ready.
    conf |= 0x1;
    (*controller).cc.write(conf);
    ctlr.wait_ready().unwrap();

    // Mask all interrupts.
    (*controller).intms.write(!0);

    // Query the Identify command for the controller and the NVM command set
    // determine optimal block size

    let ioq_len = ctlr.io_queue.len() as u32 - 1;
    let comq_addr = ctlr.io_queue.comq.addr() as u64;
    todo!("Load queues, return ctlr")
}

bitflags::bitflags! {
    /// Controller Capabilities
    #[repr(transparent)]
    pub(super) struct Capabilities : u64 {
        /// NVM Command Set is supported
        ///
        /// This command set should be implemented by all (I/O) controllers and provides
        /// basic read and write functionality.
        const NVM_COMMAND_SET   = 1 << 37;
        const BOOT_PARTITIONS   = 1 << 45;
    }
}

impl Capabilities {
    pub fn new(bits: u64) -> Capabilities {
        Self::from_bits_retain(bits)
    }

    /// Returns the controller timeout
    ///
    /// This is the maximum amount of time the driver should wait for [`Status::RDY`] to
    /// change state after `CC.EN` changes state.
    pub fn timeout(&self) -> Duration {
        let ms = self.bits() >> 24 & 0xff;
        // The value reported is in units of 500ms.

        Duration::from_millis(ms * 500)
    }

    pub fn doorbell_stride(&self) -> usize {
        let dstrd = (self.bits() >> 32) & 0xf;
        4 << dstrd
    }

    /// Returns the minimum host page size supported by the controller
    pub fn min_page_size(&self) -> usize {
        let mpsmin = (self.bits() >> 48) & 0xf;
        0x1000 << mpsmin
    }

    /// Returns the maximum host page size supported by the controller
    pub fn max_page_size(&self) -> usize {
        let mpsmax = (self.bits() >> 52) & 0xf;
        0x1000 << mpsmax
    }
}

bitflags::bitflags! {
    /// Controller Status
    #[repr(transparent)]
    pub struct Status : u32 {
        /// Controller Ready
        ///
        /// This flag is set when the controller is ready to process commands.
        const RDY   = 1 << 0;
        /// Controller Fatal Status
        ///
        /// This flag is set when a fatal controller error occurs which cannot be reported
        /// in the appropriate completion queue.
        const CFS   = 1 << 1;
    }
}

impl Status {
    const fn new(bits: u32) -> Status {
        // SAFETY: We want to keep undefined fields.
        Self::from_bits_retain(bits)
    }
}
