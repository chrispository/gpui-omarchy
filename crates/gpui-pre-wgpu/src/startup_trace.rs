//! Opt-in startup spans on Linux's CLOCK_MONOTONIC, shared with compositor and perf timestamps.
use std::{io::Write, sync::OnceLock};

pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("FLEA_STARTUP_TRACE").as_deref() == Ok("1"))
}

unsafe extern "C" {
    fn clock_gettime(clock: std::ffi::c_int, time: *mut [std::ffi::c_long; 2]) -> std::ffi::c_int;
}

pub fn now_ns() -> u128 {
    let mut time = [0; 2];
    // SAFETY: time is writable and correctly laid out for Linux clock_gettime; 1 is CLOCK_MONOTONIC.
    if unsafe { clock_gettime(1, &mut time) } != 0 {
        return 0;
    }
    time[0] as u128 * 1_000_000_000 + time[1] as u128
}

fn emit(stage: &str, start: u128, end: u128) {
    let _ = writeln!(
        std::io::stderr().lock(),
        "FLEA_STARTUP {{\"pid\":{},\"stage\":\"{}\",\"start_ns\":{},\"end_ns\":{},\"duration_ms\":{:.6}}}",
        std::process::id(), stage, start, end, end.saturating_sub(start) as f64 / 1_000_000.
    );
}

pub fn mark(stage: &'static str) {
    if enabled() {
        at(stage, now_ns());
    }
}

pub fn at(stage: &'static str, timestamp_ns: u128) {
    if enabled() {
        emit(stage, timestamp_ns, timestamp_ns);
    }
}

pub struct Span {
    stage: &'static str,
    start: Option<u128>,
}

pub fn span(stage: &'static str) -> Span {
    Span {
        stage,
        start: enabled().then(now_ns),
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            emit(self.stage, start, now_ns());
        }
    }
}
