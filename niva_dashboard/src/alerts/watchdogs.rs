use crate::hardware::hw_providers::HWInput;
use crate::hardware::sensor_manager::SensorManager;
use crate::page_framework::events::SmartEventSender;
use crate::page_framework::events::UIEvent;

// Watchdog for a particular sensor input
// Monitors the sensor value and triggers an alert if it exceeds a threshold
// Alert consists of a string message and a timeout duration
// Alerts manager will catch the event and handle alert display and timeout
pub enum WatchdogSeverity {
    Warning,
    Critical,
}

pub struct Watchdog {
    hw_input: HWInput,
    alert_message: String,
    severity: WatchdogSeverity,
    timeout_ms: u32,
    event_sender: SmartEventSender,
}

impl Watchdog {
    pub fn new(hw_input: HWInput, alert_message: String, severity: WatchdogSeverity, timeout_ms: u32, event_sender: SmartEventSender) -> Self {
        Self { hw_input, alert_message, severity, timeout_ms, event_sender }
    }

    pub fn check(&self, sensor_manager: &SensorManager) {
        let sensor_value = sensor_manager.get_sensor_value(&self.hw_input);
        if let Some(value) = sensor_value {
            let alert_condition = match self.severity {
                WatchdogSeverity::Warning => value.is_warning(),
                WatchdogSeverity::Critical => value.is_critical(),
            };
            if alert_condition {
                let event = UIEvent::AlertTriggered(self.hw_input.clone(),
                                                    self.alert_message.clone(),
                                                    self.timeout_ms);
                self.event_sender.send(event);
            }
        }
    }
}