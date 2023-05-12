pub use core::time::*;

pub fn sleep(duration: Duration) {
    let micros = duration.as_micros() as u64;

    let cur_ts = crate::raw_calls::current_ts();

    crate::raw_calls::await_ts(cur_ts + micros);
}