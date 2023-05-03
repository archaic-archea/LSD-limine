use core::sync::atomic::{AtomicU64, Ordering};

pub static TIMER_SPEED: AtomicU64 = AtomicU64::new(u64::MAX);

pub enum Unit {
    Weeks(u64),
    Days(u64),
    Hours(u64),
    Hectoseconds(u64),
    Minutes(u64),
    Decaseconds(u64),
    Seconds(u64),
    Deciseconds(u64),
    Centiseconds(u64),
    MilliSeconds(u64),
    MicroSeconds(u64),
    Ticks(u64),
}

impl Unit {
    pub fn ticks(&self) -> u64 {
        let timer_speed = TIMER_SPEED.load(Ordering::Relaxed);

        match self {
            Unit::Weeks(wks) =>         {timer_speed * wks * 604800},
            Unit::Days(days) =>         {timer_speed * days * 86400},
            Unit::Hours(hrs) =>         {timer_speed * hrs * 3600},
            Unit::Hectoseconds(hs) =>   {timer_speed * hs * 100},
            Unit::Minutes(mins) =>      {timer_speed * mins * 60},
            Unit::Decaseconds(ds) =>    {timer_speed * ds * 10},
            Unit::Seconds(secs) =>      {timer_speed * secs},
            Unit::Deciseconds(dis) =>   {timer_speed * dis / 10},
            Unit::Centiseconds(cs) =>   {timer_speed * cs / 100},
            Unit::MilliSeconds(ms) =>   {timer_speed * ms / 1000},
            Unit::MicroSeconds(us) =>   {timer_speed * us / 1000000},
            Unit::Ticks(ticks) =>       {*ticks},
        }
    }

    pub fn wait(&self) -> Result<(), sbi::SbiError> {
        let ticks = self.ticks();
        let time = crate::arch::regs::Time::get();
        sbi::timer::set_timer(ticks + time)?;

        Ok(())
    }
}