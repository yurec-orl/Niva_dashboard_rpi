#![allow(dead_code)]
//! Standalone CSV log of heading/position/speed fields for offline validation against a known
//! route -- overlay `lat`/`lon` on a map and check `fused_heading_deg`/`ins_heading_deg` track
//! the road direction, and that `gnss_speed_kmh`/`logical_speed_kmh` agree. Deliberately
//! separate from flexi_logger's `niva_dashboard.log` (tabular data for a spreadsheet/GIS tool,
//! not an event log) and from `heading_fusion_sensor`'s persisted-heading JSON (that's a single
//! last-known-value, not a time series).

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// 2 Hz -- enough to line up against road curvature/turns without ballooning the file over a
/// long drive.
const LOG_INTERVAL: Duration = Duration::from_millis(500);

/// Appends one row per `maybe_log` call, rate-limited to `LOG_INTERVAL`. Every field is
/// independently optional (blank in the CSV when absent) so a row is still written on a steady
/// time grid even while, say, GNSS has no fix yet -- that keeps rows aligned to wall-clock time
/// for later comparison against a GPX track instead of silently dropping samples.
pub struct NavValidationLog {
    writer: Option<BufWriter<File>>,
    last_log: Option<Instant>,
}

impl NavValidationLog {
    pub fn new() -> Self {
        let path = Self::default_log_path();
        NavValidationLog { writer: Self::open(&path), last_log: None }
    }

    fn default_log_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        PathBuf::from(format!("{home}/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/Logs/nav_validation.csv"))
    }

    fn open(path: &std::path::Path) -> Option<BufWriter<File>> {
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                log::warn!("Nav validation log: failed to create log dir {:?}: {}", parent, e);
                return None;
            }
        }
        // Header is written only for a brand-new file -- .append() across restarts keeps
        // accumulating one long time series instead of starting a fresh file every run.
        let is_new = !path.exists();
        let file = match OpenOptions::new().create(true).append(true).open(path) {
            Ok(f) => f,
            Err(e) => {
                log::warn!("Nav validation log: failed to open {:?}: {}", path, e);
                return None;
            }
        };
        let mut writer = BufWriter::new(file);
        if is_new {
            if let Err(e) = writeln!(
                writer,
                "unix_time_secs,ins_heading_deg,fused_heading_deg,lat,lon,gnss_course_deg,gnss_speed_kmh,logical_speed_kmh"
            ) {
                log::warn!("Nav validation log: failed to write header: {}", e);
            }
        }
        Some(writer)
    }

    /// `ins_heading_deg` is the raw BNO085 Game Rotation Vector (pre-GNSS-correction), the same
    /// source `heading_fusion_sensor` dead-reckons from. `fused_heading_deg` is the dashboard's
    /// displayed heading (HwHeading) -- comparing the two shows how much the GNSS corrections
    /// are actually pulling the INS track. `gnss_course_deg`/`gnss_speed_kmh` are the receiver's
    /// own course-over-ground and speed, for cross-checking against `logical_speed_kmh` (the
    /// wheel-pulse sensor) and the fused/INS headings.
    pub fn maybe_log(
        &mut self,
        ins_heading_deg: Option<f32>,
        fused_heading_deg: Option<f32>,
        lat: Option<f64>,
        lon: Option<f64>,
        gnss_course_deg: Option<f32>,
        gnss_speed_kmh: Option<f32>,
        logical_speed_kmh: Option<f32>,
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

        let line = format!(
            "{:.3},{},{},{},{},{},{},{}\n",
            unix_time_secs,
            fmt32(ins_heading_deg),
            fmt32(fused_heading_deg),
            fmt64(lat),
            fmt64(lon),
            fmt32(gnss_course_deg),
            fmt32(gnss_speed_kmh),
            fmt32(logical_speed_kmh),
        );

        if let Err(e) = writer.write_all(line.as_bytes()) {
            log::warn!("Nav validation log: write failed: {}", e);
            return;
        }
        // Flush every row -- only 2 Hz, and this is diagnostic data meant to survive an unclean
        // shutdown (same rationale as the main log's per-run rotation).
        if let Err(e) = writer.flush() {
            log::warn!("Nav validation log: flush failed: {}", e);
        }
    }
}
