use core::{sync::atomic::{AtomicU64, Ordering}, arch::asm, time::Duration};

pub static TIMER_SPEED: AtomicU64 = AtomicU64::new(u64::MAX);

#[derive(Debug)]
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

    /// .000000001 seconds
    //NanoSeconds(u64),

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

fn get_monotonic_count() -> u64 {
    let count: u64;

    unsafe {
        asm!("rdtime {}", out(reg) count, options(nomem, nostack, preserves_flags));
    }

    count
}

const MICROS_PER_SECOND: u64 = 1000000;

fn timebase_frequency() -> u64 {
    TIMER_SPEED.load(Ordering::Relaxed)
}

#[derive(Clone, Copy)]
pub struct Instant(u64);

impl Instant {
    pub fn now() -> Instant {
        Self(get_monotonic_count())
    }

    pub fn duration_since(self, earlier: Instant) -> Duration {
        self.checked_duration_since(earlier).unwrap_or_default()
    }

    pub fn checked_duration_since(self, earlier: Instant) -> Option<Duration> {
        let freq = timebase_frequency();
        let ticks_per_micro = (freq + MICROS_PER_SECOND - 1) / MICROS_PER_SECOND;

        let diff = self.0.checked_sub(earlier.0)?;

        let secs = diff / freq;
        let rems = diff % freq;
        let nanos = (rems / ticks_per_micro) * 1000;

        Some(Duration::new(secs, nanos as u32))
    }
}

impl core::ops::Sub for Instant {
    type Output = Duration;

    fn sub(self, rhs: Self) -> Self::Output {
        self.duration_since(rhs)
    }
}

#[derive(Clone, Copy)]
pub struct Timeout {
    start: Instant,
    duration: Duration,
}

impl Timeout {
    pub fn start(duration: Duration) -> Timeout {
        Timeout {
            start: Instant::now(),
            duration,
        }
    }

    pub fn expired(&self) -> bool {
        Instant::now() - self.start >= self.duration
    }
}