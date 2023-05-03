use core::sync::atomic::{AtomicU64, Ordering};

pub static TIMER_SPEED: AtomicU64 = AtomicU64::new(u64::MAX);

pub enum Unit {
    /// 604800 seconds
    /// 10080 minutes
    /// 168 hours
    /// 7 days
    Weeks(u64),

    /// 86400 seconds
    /// 1440 minutes
    /// 24 hours
    Days(u64),

    /// 3600 seconds
    /// 60 minutes
    Hours(u64),

    /// 100 seconds
    Hectoseconds(u64),

    /// 60 seconds
    Minutes(u64),
    
    /// 10 seconds
    Decaseconds(u64),

    /// Base unit of time
    Seconds(u64),

    /// .1 seconds
    Deciseconds(u64),

    /// .01 seconds
    Centiseconds(u64),

    /// .001 seconds
    MilliSeconds(u64),

    /// .000001 seconds
    MicroSeconds(u64),

    /// 1 tick on the hardware timer
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

    pub fn set(&self) -> Result<(), sbi::SbiError> {
        let ticks = self.ticks();
        let time = crate::arch::regs::Time::get();
        sbi::timer::set_timer(ticks + time)?;

        Ok(())
    }
}