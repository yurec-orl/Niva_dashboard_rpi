//! Monitors a single hardware sensor input and reports when it should raise an alert.
//!
//! A `Watchdog` does not create or manage `Alert`s itself — `AlertManager` polls each
//! registered watchdog via `check()` every cycle, and constructs an `Alert` from the
//! watchdog's message/severity/timeouts the first time `check()` returns true. Each
//! watchdog maps to at most one alert at a time (see `AlertManager::check_watchdogs`).
use crate::alerts::alert_manager::Severity;
use crate::hardware::hw_providers::HWInput;
use crate::hardware::sensor_manager::SensorManager;

pub struct Watchdog {
    hw_input: HWInput,
    alert_message: String,
    severity: Severity,
    alert_display_timeout: Option<std::time::Duration>,     // For how long to display the alert
    alert_remove_timeout: Option<std::time::Duration>,      // Inactive alert stays in queue for this long before removal
                                                            // to prevent alert flooding. None means no timeout -> alert removed immediately.
    trigger_start_time: Option<std::time::Instant>,
    trigger_duration: Option<std::time::Duration>,          // Duration the condition must persist to trigger an alert
}

impl Watchdog {
    /// Creates a watchdog for `hw_input`, firing `alert_message` at `severity` once the
    /// condition has persisted for `trigger_duration` (or immediately if `None`).
    /// `alert_display_timeout`/`alert_remove_timeout` are passed through unchanged to
    /// the `Alert` this watchdog eventually produces — see their accessor docs below
    /// for exact meaning.
    pub fn new(hw_input: HWInput, alert_message: String, severity: Severity,
               alert_display_timeout: Option<std::time::Duration>, alert_remove_timeout: Option<std::time::Duration>,
               trigger_duration: Option<std::time::Duration>) -> Self {
        Self { hw_input, alert_message, severity, alert_display_timeout,
               alert_remove_timeout, trigger_start_time: None, trigger_duration }
    }

    /// Polls `sensor_manager` for this watchdog's input and returns true the moment its
    /// condition has persisted for at least `trigger_duration`.
    ///
    /// Reads the current value for `hw_input`, evaluates `is_warning()`/`is_critical()`
    /// depending on `severity`, and tracks how long the condition has held continuously
    /// (resetting the timer as soon as it clears). Returns false while the sensor value
    /// is unavailable, the condition isn't met, or the persistence delay hasn't elapsed
    /// yet. Called once per polling cycle by `AlertManager::check_watchdogs`.
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

    /// Returns the hardware input this watchdog monitors.
    pub fn hw_input(&self) -> HWInput {
        self.hw_input
    }

    /// Returns the message the resulting alert will display.
    pub fn message(&self) -> &String {
        &self.alert_message
    }

    /// Returns the severity the resulting alert will have.
    pub fn severity(&self) -> Severity {
        self.severity
    }

    /// How long the alert is displayed on screen before being automatically hidden.
    /// `None` means it stays displayed until manually suppressed.
    pub fn alert_display_timeout(&self) -> Option<std::time::Duration> {
        self.alert_display_timeout
    }

    /// How long the alert stays in the queue after it was suppressed/hidden and is no
    /// longer visible. A suppressed alert that hasn't been removed from the queue
    /// prevents alerts of the same type from triggering, which is what protects against
    /// alert flooding. `None` means no timeout — the alert is removed immediately.
    pub fn alert_remove_timeout(&self) -> Option<std::time::Duration> {
        self.alert_remove_timeout
    }
}