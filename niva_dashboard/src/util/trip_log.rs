#![allow(dead_code)]
//! Per-trip CSV telemetry log: every logical sensor value plus a handful of pre-fusion/raw
//! GNSS fields kept for cross-checking (see `LOGGED_INPUTS` and the extra columns in
//! `maybe_log`), written at 2 Hz for the lifetime of the dashboard process. One file per trip
//! -- unlike `niva_dashboard.log` (flexi_logger, an event log) there's no reason to keep
//! appending across restarts here, so each run gets its own file, rotated out once 30 trips
//! have accumulated.
//!
//! `trip_CURRENT.log` is the active file. On a clean shutdown it's closed and renamed to
//! `trip_<YYYYMMDD_HHMMSS>.log` if the system clock has been set from GNSS at some point this
//! run (see `util::gnss_time_sync::clock_synced_from_gnss`), or `trip_<NNNN>.log` (NNNN from a
//! counter persisted in `State/trip_counter.json`, mirroring `heading_fusion_sensor`'s
//! persisted-heading JSON) when it hasn't -- a wall-clock name is only meaningful once the
//! clock is known-good. A `trip_CURRENT.log` already on disk at startup means the previous run
//! didn't shut down cleanly (crash, power loss); it's finalized under the counter scheme
//! immediately, since whether the clock was GNSS-trustworthy back when *that* run ended isn't
//! knowable now.

use crate::hardware::hw_providers::HWInput;
use crate::hardware::sensor_manager::SensorManager;
use crate::hardware::sensor_value::SensorValue;
use crate::util::gnss_time_sync;

use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// 2 Hz -- same rate as the validation log this replaces, enough to line up against road
/// curvature/turns without ballooning the file over a long drive.
const LOG_INTERVAL: Duration = Duration::from_millis(500);

/// Trip files to keep on disk; oldest beyond this (by mtime) are deleted once a trip finalizes.
const MAX_RETAINED_TRIPS: usize = 30;

/// Logical sensors worth recording as trip telemetry. Deliberately excludes the MFD buttons
/// (HwButton0..7), the master warning button, and the test/bench-mode fake inputs
/// (HwTestAlertInput, HwBenchTestInput) -- those are UI/test state, not vehicle telemetry.
/// Link-health pseudo-sensors (HwAdcLink etc.) are kept: they explain gaps in the other
/// columns rather than being noise themselves.
const LOGGED_INPUTS: &[HWInput] = &[
    HWInput::Hw12v,
    HWInput::HwFuelLvl,
    HWInput::HwOilPress,
    HWInput::HwEngineCoolantTemp,
    HWInput::HwBrakeFluidLvlLow,
    HWInput::HwCharge,
    HWInput::HwCheckEngine,
    HWInput::HwDiffLock,
    HWInput::HwExtLights,
    HWInput::HwFuelLvlLow,
    HWInput::HwHighBeam,
    HWInput::HwInstrIllum,
    HWInput::HwOilPressLow,
    HWInput::HwParkBrake,
    HWInput::HwSpeed,
    HWInput::HwTacho,
    HWInput::HwTurnSignal,
    HWInput::HwAdcLink,
    HWInput::HwUPSCurrent,
    HWInput::HwUPSChargeState,
    HWInput::HwUPSLink,
    HWInput::HwGnssSpeed,
    HWInput::HwGnssMovingHeading,
    HWInput::HwGnssAltitude,
    HWInput::HwGnssSatellites,
    HWInput::HwGnssFixQuality,
    HWInput::HwGnssLink,
    HWInput::HwBno085Heading,
    HWInput::HwBno085Link,
    HWInput::HwTempOut,
    HWInput::HwTempInt,
    HWInput::HwHeading,
    HWInput::HwHeadingConfidence,
    HWInput::HwHeadingAccuracy,
    HWInput::HwDeadReckoningElapsed,
];

/// Strips the `Hw` prefix off the variant's `Debug` name for a readable CSV column header
/// (e.g. `HwFuelLvl` -> `FuelLvl`).
fn column_name(input: HWInput) -> String {
    format!("{:?}", input).trim_start_matches("Hw").to_string()
}

#[derive(Serialize, Deserialize)]
struct TripCounter {
    next_number: u32,
}

pub struct TripLog {
    writer: Option<BufWriter<File>>,
    last_log: Option<Instant>,
    logs_dir: PathBuf,
    current_path: PathBuf,
    counter_path: PathBuf,
}

impl TripLog {
    pub fn new() -> Self {
        let logs_dir = Self::logs_dir();
        let current_path = logs_dir.join("trip_CURRENT.log");
        let counter_path = Self::state_dir().join("trip_counter.json");

        if current_path.exists() {
            log::warn!(
                "Trip log: found leftover {:?} from an unclean shutdown, finalizing it",
                current_path
            );
            // Not `clock_synced_from_gnss()`: that reflects *this* run's clock, not whatever
            // was true when the previous run abandoned the file -- unknowable now, so always
            // fall back to the counter scheme for a leftover file.
            Self::finalize(&current_path, &counter_path, &logs_dir, false);
        }

        TripLog {
            writer: Self::open(&current_path),
            last_log: None,
            logs_dir,
            current_path,
            counter_path,
        }
    }

    fn logs_dir() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        PathBuf::from(format!("{home}/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/Logs"))
    }

    fn state_dir() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        PathBuf::from(format!("{home}/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/State"))
    }

    fn header() -> String {
        let mut header = String::from("unix_time_secs,lat,lon,ins_heading_deg,gnss_course_deg,gnss_speed_kmh");
        for input in LOGGED_INPUTS {
            header.push(',');
            header.push_str(&column_name(*input));
        }
        header
    }

    fn open(path: &Path) -> Option<BufWriter<File>> {
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                log::warn!("Trip log: failed to create log dir {:?}: {}", parent, e);
                return None;
            }
        }
        // `current_path` is always absent by this point -- a leftover from the previous run
        // was already finalized (renamed away) above -- so this always starts a fresh file.
        let file = match OpenOptions::new().create(true).write(true).truncate(true).open(path) {
            Ok(f) => f,
            Err(e) => {
                log::warn!("Trip log: failed to open {:?}: {}", path, e);
                return None;
            }
        };
        let mut writer = BufWriter::new(file);
        if let Err(e) = writeln!(writer, "{}", Self::header()) {
            log::warn!("Trip log: failed to write header: {}", e);
        }
        Some(writer)
    }

    /// `ins_heading_deg` is the raw BNO085 Game Rotation Vector (pre-GNSS-correction) --
    /// compare against the `Heading`/`HeadingConfidence` columns (the fused value, pulled from
    /// `sensors` like every other logical field) to see how much GNSS correction is actually
    /// pulling the INS track. `gnss_course_deg`/`gnss_speed_kmh` are the receiver's own
    /// course-over-ground/speed, direct from the NMEA fix -- cross-check against the
    /// `GnssMovingHeading`/`GnssSpeed`/`Speed` columns (each already smoothed/converted through
    /// its own sensor chain). None of these three are logical `HWInput`s, so they're passed in
    /// explicitly rather than pulled from `sensors`.
    pub fn maybe_log(
        &mut self,
        sensors: &SensorManager,
        ins_heading_deg: Option<f32>,
        lat: Option<f64>,
        lon: Option<f64>,
        gnss_course_deg: Option<f32>,
        gnss_speed_kmh: Option<f32>,
    ) {
        let now = Instant::now();
        if let Some(last) = self.last_log {
            if now.duration_since(last) < LOG_INTERVAL {
                return;
            }
        }
        self.last_log = Some(now);

        let Some(writer) = &mut self.writer else { return };

        let unix_time_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);

        fn fmt32(v: Option<f32>) -> String { v.map(|v| format!("{:.2}", v)).unwrap_or_default() }
        fn fmt64(v: Option<f64>) -> String { v.map(|v| format!("{:.6}", v)).unwrap_or_default() }
        fn fmt_sensor(v: Option<&SensorValue>) -> String {
            match v {
                Some(v) if v.is_valid() => format!("{:.3}", v.as_f32()),
                _ => String::new(),
            }
        }

        let mut line = format!(
            "{:.3},{},{},{},{},{}",
            unix_time_secs,
            fmt64(lat),
            fmt64(lon),
            fmt32(ins_heading_deg),
            fmt32(gnss_course_deg),
            fmt32(gnss_speed_kmh),
        );
        for input in LOGGED_INPUTS {
            line.push(',');
            line.push_str(&fmt_sensor(sensors.get_sensor_value(input)));
        }
        line.push('\n');

        if let Err(e) = writer.write_all(line.as_bytes()) {
            log::warn!("Trip log: write failed: {}", e);
            return;
        }
        // Flush every row -- only 2 Hz, and this is diagnostic data meant to survive an
        // unclean shutdown (same rationale as the main log's per-run rotation).
        if let Err(e) = writer.flush() {
            log::warn!("Trip log: flush failed: {}", e);
        }
    }

    /// Closes and renames `current_path`, then prunes retained trips down to
    /// `MAX_RETAINED_TRIPS`. `synced` decides the naming scheme -- see the module doc.
    fn finalize(current_path: &Path, counter_path: &Path, logs_dir: &Path, synced: bool) {
        if !current_path.exists() {
            return;
        }

        let new_name = if synced {
            let unix_secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            Self::timestamp_name(unix_secs)
        } else {
            format!("trip_{:04}.log", Self::next_counter(counter_path))
        };

        let dest = logs_dir.join(&new_name);
        if let Err(e) = fs::rename(current_path, &dest) {
            log::warn!("Trip log: failed to rename {:?} to {:?}: {}", current_path, dest, e);
            return;
        }
        log::info!("Trip log: finalized trip as {:?}", dest);

        Self::prune_old_trips(logs_dir);
    }

    fn timestamp_name(unix_secs: u64) -> String {
        let days = (unix_secs / 86400) as i64;
        let secs_of_day = unix_secs % 86400;
        let (year, month, day) = gnss_time_sync::civil_from_days(days);
        let (hour, minute, second) = (secs_of_day / 3600, (secs_of_day / 60) % 60, secs_of_day % 60);
        format!("trip_{:04}{:02}{:02}_{:02}{:02}{:02}.log", year, month, day, hour, minute, second)
    }

    /// Reads, increments, and persists the trip counter, returning the number this trip
    /// should use. Starts at 1 if the counter file is missing or unreadable.
    fn next_counter(counter_path: &Path) -> u32 {
        let current = fs::read_to_string(counter_path)
            .ok()
            .and_then(|s| serde_json::from_str::<TripCounter>(&s).ok())
            .map(|c| c.next_number)
            .unwrap_or(1);

        if let Some(parent) = counter_path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                log::warn!("Trip log: failed to create state dir {:?}: {}", parent, e);
            }
        }
        let next = TripCounter { next_number: current.wrapping_add(1) };
        if let Ok(json) = serde_json::to_string(&next) {
            if let Err(e) = fs::write(counter_path, json) {
                log::warn!("Trip log: failed to persist trip counter to {:?}: {}", counter_path, e);
            }
        }

        current
    }

    /// Deletes the oldest finalized trip files (by mtime, not filename -- timestamp-named and
    /// counter-named files don't share a single sortable scheme) once more than
    /// `MAX_RETAINED_TRIPS` are present. `trip_CURRENT.log` is never a candidate here since
    /// this only runs right after it's been renamed away.
    fn prune_old_trips(logs_dir: &Path) {
        let Ok(entries) = fs::read_dir(logs_dir) else { return };

        let mut trips: Vec<(PathBuf, SystemTime)> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("trip_") && n.ends_with(".log") && n != "trip_CURRENT.log")
            })
            .filter_map(|p| fs::metadata(&p).and_then(|m| m.modified()).ok().map(|t| (p, t)))
            .collect();

        if trips.len() <= MAX_RETAINED_TRIPS {
            return;
        }

        trips.sort_by_key(|(_, mtime)| *mtime);
        for (path, _) in &trips[..trips.len() - MAX_RETAINED_TRIPS] {
            if let Err(e) = fs::remove_file(path) {
                log::warn!("Trip log: failed to prune old trip {:?}: {}", path, e);
            } else {
                log::info!("Trip log: pruned old trip {:?}", path);
            }
        }
    }
}

impl Drop for TripLog {
    fn drop(&mut self) {
        // Drop the writer first so the file is closed (and its buffered tail flushed) before
        // the rename below.
        if let Some(mut writer) = self.writer.take() {
            let _ = writer.flush();
        }
        Self::finalize(&self.current_path, &self.counter_path, &self.logs_dir, gnss_time_sync::clock_synced_from_gnss());
    }
}
