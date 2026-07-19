use crate::alerts::alert_manager::Severity;
use crate::hardware::hw_providers::HWInput;
use crate::hardware::sensor_manager::SensorManager;

// Watchdog for a particular sensor input.
// Monitors the sensor value and triggers an alert 
// if it exceeds a threshold for a specified duration.
// Alert consists of a string message and a timeout duration.
// Alerts manager will catch the event and handle alert display and timeout.
pub struct Watchdog {
    hw_input: HWInput,
    alert_message: String,
    severity: Severity,
    alert_display_timeout: Option<std::time::Duration>,      // For how long to display the alert
    alert_remove_timeout: Option<std::time::Duration>,       // Inactive alert stays in queue for this long before removal
                                                // to prevent alert flooding.
    trigger_start_time: Option<std::time::Instant>,
    trigger_duration: Option<std::time::Duration>, // Duration the condition must persist to trigger an alert
}

impl Watchdog {
    pub fn new(hw_input: HWInput, alert_message: String, severity: Severity,
               alert_display_timeout: Option<std::time::Duration>, alert_remove_timeout: Option<std::time::Duration>,
               trigger_duration: Option<std::time::Duration>) -> Self {
        Self { hw_input, alert_message, severity, alert_display_timeout,
               alert_remove_timeout, trigger_start_time: None, trigger_duration }
    }

    // Return true when the watchdog detects a condition that should trigger an alert
    pub fn check(&mut self, sensor_manager: &SensorManager) -> bool {
        let sensor_value = sensor_manager.get_sensor_value(&self.hw_input);
        let trigger = if let Some(value) = sensor_value {
                match self.severity {
                    Severity::Warning => value.is_warning(),
                    Severity::Critical => value.is_critical(),
                }
            } else {
                false
            };
        if trigger {
            if let Some(trigger_duration) = self.trigger_duration {
                if let Some(start_time) = self.trigger_start_time {
                    if start_time.elapsed() >= trigger_duration {
                        return true; // Condition has persisted long enough to trigger
                    }
                } else {
                    // Start timing the trigger condition
                    self.trigger_start_time = Some(std::time::Instant::now());
                }
            } else {
                return true; // Immediate trigger if no duration specified
            }
        } else {
            // Reset if condition is not met
            self.trigger_start_time = None;
        }
        false
    }

    pub fn hw_input(&self) -> HWInput {
        self.hw_input
    }

    pub fn message(&self) -> &String {
        &self.alert_message
    }

    pub fn severity(&self) -> Severity {
        self.severity
    }

    pub fn alert_display_timeout(&self) -> Option<std::time::Duration> {
        self.alert_display_timeout
    }

    pub fn alert_remove_timeout(&self) -> Option<std::time::Duration> {
        self.alert_remove_timeout
    }
}