// Field-calibration overlay for `calibrated_analog` sensors -- see
// SENSOR_CALIBRATION_DESIGN.md, "Calibration overlay file". A separate file from
// sensor_config.json (which is hand-authored/checked in) so the field calibration UI never
// rewrites that file. One record per sensor id; a later capture overwrites the earlier one
// rather than accumulating.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub struct CalibrationRecord {
    /// The sensor's own datasheet curve output at capture time, with any *previous*
    /// value_offset already subtracted back out -- not the raw on-screen reading. Storing
    /// the curve-only value here (rather than whatever was on screen, offset included) is
    /// what keeps `offset()` correct across a second/later recalibration: each capture
    /// computes its offset relative to the pristine curve, not relative to the last
    /// capture's correction.
    pub reported: f32,
    /// What the operator set the reading to at the same instant (see Anchor-point
    /// procedure in SENSOR_CALIBRATION_DESIGN.md).
    pub true_value: f32,
}

impl CalibrationRecord {
    pub fn offset(&self) -> f32 {
        self.true_value - self.reported
    }
}

/// Lives next to sensor_config.json, but -- unlike it -- is per-vehicle runtime data, not
/// checked into the repo (gitignored).
pub fn default_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
    PathBuf::from(format!("{home}/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/niva_dashboard/sensor_calibration.json"))
}

/// A missing file is the normal "no calibration captured yet / reset to datasheet default"
/// state, not an error -- unlike sensor_config.json, this file isn't expected to always
/// exist. Malformed JSON in a file that does exist is still a fail-fast `Err`, same as
/// sensor_config.json.
pub fn load(path: &Path) -> Result<HashMap<String, CalibrationRecord>, String> {
    match std::fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents)
            .map_err(|e| format!("sensor calibration: failed to parse {path:?}: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
        Err(e) => Err(format!("sensor calibration: failed to read {path:?}: {e}")),
    }
}

/// Whole-file rewrite -- at most a handful of records (one per calibrated sensor), so no
/// need for a partial/streaming update.
pub fn save(path: &Path, records: &HashMap<String, CalibrationRecord>) -> Result<(), String> {
    let contents = serde_json::to_string_pretty(records)
        .map_err(|e| format!("sensor calibration: failed to serialize: {e}"))?;
    std::fs::write(path, contents)
        .map_err(|e| format!("sensor calibration: failed to write {path:?}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_is_true_value_minus_reported() {
        let r = CalibrationRecord { reported: 74.0, true_value: 90.0 };
        assert!((r.offset() - 16.0).abs() < 0.001);
    }

    #[test]
    fn load_missing_file_returns_empty_map() {
        let map = load(Path::new("/nonexistent/sensor_calibration.json")).expect("missing file is not an error");
        assert!(map.is_empty());
    }

    #[test]
    fn load_malformed_file_is_an_error() {
        let dir = std::env::temp_dir().join(format!("niva_calib_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sensor_calibration.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert!(load(&path).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join(format!("niva_calib_test_rt_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sensor_calibration.json");

        let mut records = HashMap::new();
        records.insert("HwEngineCoolantTemp".to_string(), CalibrationRecord { reported: 74.0, true_value: 90.0 });
        save(&path, &records).expect("save should succeed");

        let loaded = load(&path).expect("load should succeed");
        assert_eq!(loaded, records);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
