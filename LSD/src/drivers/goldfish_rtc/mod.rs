use crate::volatile::{Volatile, Read, Write};

pub static RTC: crate::SetOnce<*mut GoldfishRTC> = crate::SetOnce::new(core::ptr::null_mut());

#[repr(C)]
pub struct GoldfishRTC {
    pub time: Time,
    alarm: Alarm,
    clear_interrupt: Volatile<u8, Write>
}

#[repr(transparent)]
pub struct Time(Volatile<[u32; 2], Read>);

impl Time {
    pub fn read(&self) -> u64 {
        let low = self.0.read()[0] as u64;
        let high = self.0.read()[1] as u64;

        ((high << 32) + low) / 1_000_000_000
    }
}

#[repr(transparent)]
pub struct Alarm(Volatile<[u32; 2], Write>);

impl Alarm {
    pub fn write(&self, val: u64) {
        let low = (val & 0xFFFF_FFFF) as u32;
        let high = (val >> 32) as u32;

        let arr = [low, high];

        self.0.write(arr);
    }
}

pub struct UnixTimestamp(pub u64);

impl UnixTimestamp {
    pub fn date(&self) -> Date {
        let seconds = self.0 as usize;
        let minutes = seconds / 60;
        let hours = seconds / 3600;
        let days = (seconds % 31556926) / 86400;
        let years = seconds / 31556926;

        let months = Month::from_offset(days, (years % 4) != 0);

        Date { 
            year: years, 
            month: months, 
            day: days - months.offset((years % 4) != 0), 
            hour: hours % 24,
            minute: minutes % 60, 
            second: seconds % 60
        }
    }
}

pub struct Date {
    year: usize,
    month: Month,
    day: usize,
    hour: usize,
    minute: usize,
    second: usize
}

impl core::fmt::Debug for Date {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if f.alternate() {
            writeln!(f, "year: {}", self.year + 1970)?;
            writeln!(f, "month: {:?}", self.month)?;
            writeln!(f, "day: {}", self.day)?;
            writeln!(f, "hour: {}", self.hour)?;
            writeln!(f, "minute: {}", self.minute)?;
            write!(f, "second: {}", self.second)?;
        } else {
            write!(
                f, 
                "{} of {:?} {} {}:{}:{}", 
                self.day,
                self.month,
                self.year + 1970,
                self.hour,
                self.minute,
                self.second
            )?;
        }

        Ok(())
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(usize)]
enum Month {
    January,
    Febuary,
    March,
    April,
    May,
    June,
    July,
    August,
    September,
    October,
    November,
    December
}

impl Month {
    pub fn offset(&self, is_leap: bool) -> usize {
        let leap_offset = if is_leap {
            1
        } else {
            0
        };

        match self {
            Self::January => 0,
            Self::Febuary => 31,
            Self::March => 59 - leap_offset,
            Self::April => 90 - leap_offset,
            Self::May => 120 - leap_offset,
            Self::June => 151 - leap_offset,
            Self::July => 181 - leap_offset,
            Self::August => 212 - leap_offset,
            Self::September => 243 - leap_offset,
            Self::October => 273 - leap_offset,
            Self::November => 304 - leap_offset,
            Self::December => 334 - leap_offset
        }
    }

    pub fn from_offset(offset: usize, is_leap: bool) -> Self {
        let leap_offset = if is_leap {
            1
        } else {
            0
        };

        let jan_rang = 0..31;
        let feb_rang = 31..59 + leap_offset;
        let mar_rang = 59..90 + leap_offset;
        let apr_rang = 90..120 + leap_offset;
        let may_rang = 120..151 + leap_offset;
        let jun_rang = 151..181 + leap_offset;
        let jul_rang = 181..212 + leap_offset;
        let aug_rang = 212..243 + leap_offset;
        let sep_rang = 243..273 + leap_offset;
        let oct_rang = 273..304 + leap_offset;
        let nov_rang = 304..334 + leap_offset;
        let dec_rang = 334..365 + leap_offset;

        if jan_rang.contains(&offset) {
            return Self::January;
        } else if feb_rang.contains(&offset) {
            return Self::Febuary;
        } else if mar_rang.contains(&offset) {
            return Self::March;
        }  else if apr_rang.contains(&offset) {
            return Self::April;
        }  else if may_rang.contains(&offset) {
            return Self::May;
        }  else if jun_rang.contains(&offset) {
            return Self::June;
        }  else if jul_rang.contains(&offset) {
            return Self::July;
        }  else if aug_rang.contains(&offset) {
            return Self::August;
        }  else if sep_rang.contains(&offset) {
            return Self::September;
        }  else if oct_rang.contains(&offset) {
            return Self::October;
        }  else if nov_rang.contains(&offset) {
            return Self::November;
        }  else if dec_rang.contains(&offset) {
            return Self::December;
        } 

        panic!("Invalid offset");
    }
}