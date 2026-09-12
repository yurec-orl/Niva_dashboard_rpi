// Bench-test target values for the "settle" mode of TestADCDataProvider (see
// util::adc_data_provider) -- what DiagPage's ТЕСТ button ramps synthetic sensors toward and
// holds them at, so the calibration UI can be exercised on the bench without real senders
// wired up. A separate file from sensor_config.json (checked in) and sensor_calibration.json
// (per-vehicle field captures): this one is bench-session tuning data, not checked in.
//
// Keys are HWInput::config_name() strings (same convention as sensor_config.json's own
// "hw_input" field). Values are in the same physical domain TestADCDataProvider's settle mode
// already computes in: ohms for the three resistive senders (HwFuelLvl/HwOilPress/
// HwEngineCoolantTemp), volts for Hw12v, km/h for HwSpeed, rpm for HwTacho. A key with no
// entry here falls back to TestADCDataProvider's own built-in default for that channel.

use crate::hardware::hw_providers::HWInput;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Lives next to sensor_config.json, but -- like sensor_calibration.json -- is per-bench-
/// session data, not checked into the repo (gitignored).
pub fn default_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
    PathBuf::from(format!("{home}/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/niva_dashboard/mock_sensor_values.json"))
}

/// A missing file just means "use TestADCDataProvider's built-in defaults for everything",
/// not an error -- same as sensor_calibration.json. Malformed JSON or an unknown hw_input key
/// is still a fail-fast `Err`.
pub fn load(path: &Path) -> Result<HashMap<HWInput, f32>, String> {
    let raw: HashMap<String, f32> = match std::fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents)
            .map_err(|e| format!("mock sensor values: failed to parse {path:?}: {e}"))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(e) => return Err(format!("mock sensor values: failed to read {path:?}: {e}")),
    };

    raw.into_iter()
        .map(|(name, value)| {
            HWInput::from_config_name(&name)
                .map(|input| (input, value))
                .ok_or_else(|| format!("mock sensor values: unknown hw_input '{name}'"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_file_returns_empty_map() {
        let map = load(Path::new("/nonexistent/mock_sensor_values.json")).expect("missing file is not an error");
        assert!(map.is_empty());
    }

    #[test]
    fn load_malformed_file_is_an_error() {
        let dir = std::env::temp_dir().join(format!("niva_mock_sensor_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mock_sensor_values.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert!(load(&path).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_unknown_hw_input_is_an_error() {
        let dir = std::env::temp_dir().join(format!("niva_mock_sensor_test_unknown_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mock_sensor_values.json");
        std::fs::write(&path, r#"{ "NotARealInput": 1.0 }"#).unwrap();
        assert!(load(&path).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_valid_file_maps_known_inputs() {
        let dir = std::env::temp_dir().join(format!("niva_mock_sensor_test_valid_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mock_sensor_values.json");
        std::fs::write(&path, r#"{ "HwFuelLvl": 45.0, "Hw12v": 13.8 }"#).unwrap();

        let map = load(&path).expect("valid file should load");
        assert_eq!(map.get(&HWInput::HwFuelLvl), Some(&45.0));
        assert_eq!(map.get(&HWInput::Hw12v), Some(&13.8));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
