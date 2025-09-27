use crate::alerts::alert_manager::Severity;
use crate::hardware::hw_providers::HWInput;
use crate::hardware::sensor_manager::SensorManager;

// Watchdog for a particular sensor input
// Monitors the sensor value and triggers an alert if it exceeds a threshold
// Alert consists of a string message and a timeout duration
// Alerts manager will catch the event and handle alert display and timeout
pub struct Watchdog {
    hw_input: HWInput,
    alert_message: String,
    severity: Severity,
    timeout_ms: u32,
}

impl Watchdog {
    pub fn new(hw_input: HWInput, alert_message: String, severity: Severity, timeout_ms: u32) -> Self {
        Self { hw_input, alert_message, severity, timeout_ms }
    }

    // Return true when the watchdog detects a condition that should trigger an alert
    pub fn check(&self, sensor_manager: &SensorManager) -> bool {
        let sensor_value = sensor_manager.get_sensor_value(&self.hw_input);
        if let Some(value) = sensor_value {
            match self.severity {
                Severity::Warning => value.is_warning(),
                Severity::Critical => value.is_critical(),
            };
        }
        false
    }

    pub fn message(&self) -> &String {
        &self.alert_message
    }

    pub fn severity(&self) -> Severity {
        self.severity
    }

    pub fn timeout_ms(&self) -> u32 {
        self.timeout_ms
    }
}