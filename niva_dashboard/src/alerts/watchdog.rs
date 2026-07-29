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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::hw_providers::HWAnalogProvider;
    use crate::hardware::sensor_manager::{SensorAnalogInputChain, SensorManager};
    use crate::hardware::sensor_value::ValueConstraints;
    use crate::hardware::sensors::GenericAnalogSensor;

    // Returns a fixed raw value regardless of input, so tests can control exactly what
    // Watchdog::check sees without depending on TestAnalogDataProvider's time-based pattern.
    struct FixedAnalogDataProvider {
        input: HWInput,
        raw_value: u16,
    }

    impl HWAnalogProvider for FixedAnalogDataProvider {
        fn input(&self) -> HWInput { self.input }
        fn read_analog(&self, _input: HWInput) -> Result<u16, String> {
            Ok(self.raw_value)
        }
    }

    // Builds a SensorManager with a single passthrough (scale 1.0, no processors) analog
    // chain for `input`, reporting exactly `raw_value` with warning_high=70, critical_high=90.
    fn manager_with_fixed_value(input: HWInput, raw_value: u16) -> SensorManager {
        let mut manager = SensorManager::new();
        let chain = SensorAnalogInputChain::new(
            Box::new(FixedAnalogDataProvider { input, raw_value }),
            vec![],
            Box::new(GenericAnalogSensor::new(
                "watchdog_test".to_string(), "Watchdog Test".to_string(), "".to_string(),
                ValueConstraints::analog_with_thresholds(0.0, 100.0, None, None, Some(70.0), Some(90.0)),
                1.0,
            )),
        );
        manager.add_analog_sensor_chain(chain);
        manager.read_all_sensors().expect("fixed provider read should never fail");
        manager
    }

    #[test]
    fn test_check_triggers_immediately_without_trigger_duration() {
        let manager = manager_with_fixed_value(HWInput::HwEngineCoolantTemp, 95); // critical: >= 90
        let mut watchdog = Watchdog::new(
            HWInput::HwEngineCoolantTemp, "overheating".to_string(), Severity::Critical,
            None, None, None,
        );

        assert!(watchdog.check(&manager), "no trigger_duration means the condition fires on the first check");
    }

    #[test]
    fn test_check_does_not_trigger_when_condition_not_met() {
        let manager = manager_with_fixed_value(HWInput::HwEngineCoolantTemp, 50); // below both thresholds
        let mut watchdog = Watchdog::new(
            HWInput::HwEngineCoolantTemp, "overheating".to_string(), Severity::Critical,
            None, None, None,
        );

        assert!(!watchdog.check(&manager));
    }

    #[test]
    fn test_check_respects_severity_warning_vs_critical() {
        // 75 is >= warning_high (70) but below critical_high (90): warning fires, critical does not.
        let manager = manager_with_fixed_value(HWInput::HwEngineCoolantTemp, 75);

        let mut warning_watchdog = Watchdog::new(
            HWInput::HwEngineCoolantTemp, "warm".to_string(), Severity::Warning, None, None, None,
        );
        let mut critical_watchdog = Watchdog::new(
            HWInput::HwEngineCoolantTemp, "overheating".to_string(), Severity::Critical, None, None, None,
        );

        assert!(warning_watchdog.check(&manager));
        assert!(!critical_watchdog.check(&manager));
    }

    #[test]
    fn test_check_requires_condition_to_persist_for_trigger_duration() {
        let manager = manager_with_fixed_value(HWInput::HwEngineCoolantTemp, 95);
        let mut watchdog = Watchdog::new(
            HWInput::HwEngineCoolantTemp, "overheating".to_string(), Severity::Critical,
            None, None, Some(std::time::Duration::from_millis(20)),
        );

        assert!(!watchdog.check(&manager), "first check only starts the persistence timer");
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert!(watchdog.check(&manager), "condition has now persisted past trigger_duration");
    }

    #[test]
    fn test_check_resets_persistence_timer_when_condition_clears() {
        let triggering = manager_with_fixed_value(HWInput::HwEngineCoolantTemp, 95);
        let clear = manager_with_fixed_value(HWInput::HwEngineCoolantTemp, 50);
        let mut watchdog = Watchdog::new(
            HWInput::HwEngineCoolantTemp, "overheating".to_string(), Severity::Critical,
            None, None, Some(std::time::Duration::from_millis(20)),
        );

        assert!(!watchdog.check(&triggering), "starts the timer");
        assert!(!watchdog.check(&clear), "condition clears, timer resets");

        std::thread::sleep(std::time::Duration::from_millis(30));
        // Even though enough time has passed since the first check, the timer was reset by
        // the intervening clear, so a fresh trigger must restart the wait from scratch.
        assert!(!watchdog.check(&triggering), "timer restarts rather than reusing the stale start time");
    }

    #[test]
    fn test_accessors_return_constructor_values() {
        let display_timeout = Some(std::time::Duration::from_secs(5));
        let remove_timeout = Some(std::time::Duration::from_secs(10));
        let watchdog = Watchdog::new(
            HWInput::HwEngineCoolantTemp, "overheating".to_string(), Severity::Critical,
            display_timeout, remove_timeout, None,
        );

        assert_eq!(watchdog.hw_input(), HWInput::HwEngineCoolantTemp);
        assert_eq!(watchdog.message(), "overheating");
        assert!(matches!(watchdog.severity(), Severity::Critical));
        assert_eq!(watchdog.alert_display_timeout(), display_timeout);
        assert_eq!(watchdog.alert_remove_timeout(), remove_timeout);
    }
}