use core::sync::atomic::{AtomicU64, Ordering};

pub static TIMER_SPEED: AtomicU64 = AtomicU64::new(u64::MAX);

pub enum Unit {
    Weeks(u64),
    Days(u64),
    Hours(u64),
    Minutes(u64),
    Seconds(u64),
    MilliSeconds(u64),
    MicroSeconds(u64),
}

impl Unit {
    pub fn ticks(&self) -> u64 {
        let timer_speed = TIMER_SPEED.load(Ordering::Relaxed);

        match self {
            Unit::Weeks(wks) => {
                timer_speed * wks * 604800
            },
            Unit::Days(days) => {
                timer_speed * days * 86400
            },
            Unit::Hours(hrs) => {
                timer_speed * hrs * 3600
            },
            Unit::Minutes(mins) => {
                timer_speed * mins * 60
            },
            Unit::Seconds(secs) => {
                timer_speed * secs
            },
            Unit::MilliSeconds(ms) => {
                let ms = ms / 1000;

                timer_speed * ms
            },
            Unit::MicroSeconds(us) => {
                let us = us / 1000000;

                timer_speed * us
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