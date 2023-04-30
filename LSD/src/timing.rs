use core::sync::atomic::{AtomicU64, Ordering};

pub static TIMER_SPEED: AtomicU64 = AtomicU64::new(u64::MAX);

pub enum Unit {
    Seconds(u64),
}

impl Unit {
    pub fn ticks(&self) -> u64 {
        let timer_speed = TIMER_SPEED.load(Ordering::Relaxed);

        match self {
            Unit::Seconds(secs) => {
                return timer_speed * secs;
            }
        }
    }

    pub fn wait(&self) -> Result<(), sbi::SbiError> {
        let ticks = self.ticks();
        let time = crate::arch::regs::Time::get();
        sbi::timer::set_timer(ticks + time)?;

        Ok(())
    }
}