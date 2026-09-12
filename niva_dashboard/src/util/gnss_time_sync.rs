//! Syncs the system clock from GNSS UTC time. There's no RTC on this hardware (see
//! CLAUDE.md's Boot Time / Logging notes) and no guaranteed NTP path once installed in a
//! car (Ethernet is a diagnostics/dev link, not an internet uplink), so the GNSS receiver
//! -- when present and locked -- is the only trustworthy time source available at runtime.

use crate::util::gnss_data_provider::GnssFrame;
use crate::util::nmea::{GnssFix, UtcDate, UtcTime};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Re-sync cadence once a good fix is available. The Pi's oscillator drift over an hour is
/// far smaller than anything that would show up in logs -- this just bounds worst-case
/// drift for a process that runs for days.
const RESYNC_INTERVAL: Duration = Duration::from_secs(3600);
/// How often to check the GNSS frame for a fix worth trusting.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// GNSS year values outside this range mean the receiver hasn't acquired a real almanac yet
/// (cold-start default date, which still carries a valid NMEA checksum) rather than that the
/// clock is genuinely off by decades -- reject instead of stepping to nonsense.
const MIN_PLAUSIBLE_YEAR: u16 = 2024;
const MAX_PLAUSIBLE_YEAR: u16 = 2100;

/// Background thread that watches a `GnssFrame` and steps `CLOCK_REALTIME` from its fix once
/// a trustworthy time/date is seen, then re-applies it hourly. Owns nothing but the thread
/// handle -- `frame` is a cheap clone of whatever `GnssDataProvider`/`TestGnssDataProvider`
/// is already writing into, same pattern as every other GnssFrame consumer.
pub struct GnssTimeSync {
    should_stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl GnssTimeSync {
    /// Starts the sync loop. No-op (never steps the clock) until `frame` has a plausible
    /// time/date, e.g. before the receiver has decoded GPS time at all or while it's
    /// disconnected -- including under `TestGnssDataProvider`, whose synthetic fix always
    /// leaves `time`/`date` as `None`. Deliberately not gated on RMC's status_active: a
    /// receiver decodes time from a single satellite's subframe well before it has enough
    /// satellites for a position fix, so waiting for status_active would delay syncing for no
    /// reason -- see trustworthy_fix_time.
    pub fn start(frame: GnssFrame) -> Self {
        let should_stop = Arc::new(AtomicBool::new(false));
        let thread_should_stop = Arc::clone(&should_stop);

        let thread = thread::Builder::new()
            .name("gnss-time-sync".into())
            .spawn(move || Self::run_loop(&thread_should_stop, &frame))
            .ok();

        GnssTimeSync { should_stop, thread }
    }

    fn run_loop(should_stop: &AtomicBool, frame: &GnssFrame) {
        let mut last_sync: Option<Instant> = None;

        while !should_stop.load(Ordering::Relaxed) {
            let due = last_sync.is_none_or(|t| t.elapsed() >= RESYNC_INTERVAL);

            if due {
                if let Some((date, time)) = trustworthy_fix_time(&frame.fix()) {
                    match apply_clock(date, time) {
                        Ok(()) => {
                            log::info!(
                                "System clock set from GNSS: {:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
                                date.year, date.month, date.day,
                                time.hour, time.minute, time.second as u8
                            );
                            last_sync = Some(Instant::now());
                        }
                        Err(e) if e.raw_os_error() == Some(libc::EPERM) => {
                            // CAP_SYS_TIME is missing (e.g. AmbientCapabilities not applied
                            // yet -- see niva-dashboard.service, needs a real `systemctl
                            // restart`, not just a rebuild-relaunch) and won't appear without
                            // a restart of this process either way, so retrying every poll
                            // would just repeat the same failure forever. Log once and exit.
                            log::warn!(
                                "GNSS time sync disabled: clock_settime not permitted ({}). \
                                 Needs CAP_SYS_TIME (see niva-dashboard.service) -- restart \
                                 the service after fixing.", e
                            );
                            return;
                        }
                        Err(e) => {
                            // Leave last_sync alone so the next poll retries rather than
                            // waiting a full hour on what might be a transient failure.
                            log::warn!("Failed to set system clock from GNSS: {}", e);
                        }
                    }
                }
            }

            Self::sleep_while_running(should_stop, POLL_INTERVAL);
        }
    }

    fn sleep_while_running(should_stop: &AtomicBool, duration: Duration) {
        const STEP: Duration = Duration::from_millis(100);
        let mut remaining = duration;
        while remaining > Duration::ZERO && !should_stop.load(Ordering::Relaxed) {
            let step = remaining.min(STEP);
            thread::sleep(step);
            remaining -= step;
        }
    }

    pub fn stop(&mut self) {
        self.should_stop.store(true, Ordering::SeqCst);
    }
}

impl Drop for GnssTimeSync {
    fn drop(&mut self) {
        self.stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Returns the fix's date/time once both are present and pass basic sanity checks. Not
/// gated on RMC's status_active ('A'/'V') -- see `GnssTimeSync::start`'s doc comment for why
/// that would only delay syncing without adding real protection. `parse_date_ddmmyy`/
/// `parse_time` don't validate field ranges themselves, so a corrupted-but-checksum-valid
/// line could otherwise carry an out-of-range month/day/hour/minute/second; the year check
/// additionally catches a cold-start default date, which is implausible but in-range.
fn trustworthy_fix_time(fix: &GnssFix) -> Option<(UtcDate, UtcTime)> {
    let date = fix.date?;
    let time = fix.time?;
    if date.year < MIN_PLAUSIBLE_YEAR || date.year > MAX_PLAUSIBLE_YEAR {
        return None;
    }
    if date.month == 0 || date.month > 12 || date.day == 0 || date.day > 31 {
        return None;
    }
    if time.hour > 23 || time.minute > 59 || !(0.0..60.0).contains(&time.second) {
        return None;
    }
    Some((date, time))
}

/// Days since the Unix epoch for a Gregorian civil date, via Howard Hinnant's
/// `days_from_civil` algorithm -- avoids pulling in a date/time crate for this one
/// conversion. Valid for any proleptic Gregorian date; the plausible-year check above keeps
/// inputs well inside that range anyway.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn apply_clock(date: UtcDate, time: UtcTime) -> Result<(), std::io::Error> {
    let days = days_from_civil(date.year as i64, date.month as i64, date.day as i64);
    let secs_of_day = time.hour as i64 * 3600 + time.minute as i64 * 60 + time.second as i64;
    let epoch_secs = days * 86400 + secs_of_day;

    let ts = libc::timespec {
        tv_sec: epoch_secs as libc::time_t,
        tv_nsec: 0,
    };

    // SAFETY: `ts` is a fully-initialized timespec on the stack; clock_settime only reads it
    // and writes kernel time state, nothing is retained past the call.
    let rc = unsafe { libc::clock_settime(libc::CLOCK_REALTIME, &ts) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fix_with(date: UtcDate, time: UtcTime) -> GnssFix {
        GnssFix { date: Some(date), time: Some(time), ..Default::default() }
    }

    #[test]
    fn accepts_fix_without_active_status_but_with_plausible_time() {
        // Mirrors nmea::tests::parses_real_captured_rmc_with_no_fix: real hardware reports
        // accurate time on a cold-start RMC line before it has a position fix.
        let date = UtcDate { day: 29, month: 7, year: 2026 };
        let time = UtcTime { hour: 15, minute: 48, second: 49.0 };
        let mut fix = fix_with(date, time);
        fix.status_active = Some(false);
        assert_eq!(trustworthy_fix_time(&fix), Some((date, time)));
    }

    #[test]
    fn rejects_missing_time_or_date() {
        let mut fix = GnssFix::default();
        assert!(trustworthy_fix_time(&fix).is_none());
        fix.date = Some(UtcDate { day: 12, month: 9, year: 2026 });
        assert!(trustworthy_fix_time(&fix).is_none());
    }

    #[test]
    fn rejects_implausible_year() {
        let fix = fix_with(
            UtcDate { day: 6, month: 1, year: 1980 },
            UtcTime { hour: 0, minute: 0, second: 0.0 },
        );
        assert!(trustworthy_fix_time(&fix).is_none());
    }

    #[test]
    fn rejects_out_of_range_date_or_time_fields() {
        let base_date = UtcDate { day: 12, month: 9, year: 2026 };
        let base_time = UtcTime { hour: 10, minute: 0, second: 0.0 };

        assert!(trustworthy_fix_time(&fix_with(UtcDate { month: 13, ..base_date }, base_time)).is_none());
        assert!(trustworthy_fix_time(&fix_with(UtcDate { day: 32, ..base_date }, base_time)).is_none());
        assert!(trustworthy_fix_time(&fix_with(base_date, UtcTime { hour: 24, ..base_time })).is_none());
        assert!(trustworthy_fix_time(&fix_with(base_date, UtcTime { minute: 60, ..base_time })).is_none());
        assert!(trustworthy_fix_time(&fix_with(base_date, UtcTime { second: 60.0, ..base_time })).is_none());
    }

    #[test]
    fn accepts_plausible_date_regardless_of_fix_status() {
        let date = UtcDate { day: 12, month: 9, year: 2026 };
        let time = UtcTime { hour: 10, minute: 30, second: 15.0 };
        let fix = fix_with(date, time);
        assert_eq!(trustworthy_fix_time(&fix), Some((date, time)));
    }

    #[test]
    fn days_from_civil_matches_known_epoch_offsets() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2026, 9, 12), 20708);
    }
}
