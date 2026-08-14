//! Owns the set of hardware watchdogs and the alerts they've raised, and renders the
//! currently active ones to screen.
//!
//! ## Alert lifecycle
//! 1. Each cycle, `AlertManager::check_watchdogs` polls every registered `Watchdog`.
//!    The first time a watchdog's condition fires, a matching `Alert` is created and
//!    added to the queue, tagged with the watchdog's id so at most one alert exists per
//!    watchdog at a time.
//! 2. `AlertManager::render_alerts` draws every alert that is currently active (see
//!    `Alert`'s docs for what "active" means and how `display_timeout` governs it),
//!    stacked and centered on screen.
//! 3. Once an alert stops being active (its `display_timeout` elapsed, or it was
//!    suppressed), it lingers in the queue — still blocking its watchdog from raising a
//!    duplicate — until `Alert::is_expired` returns true, at which point
//!    `render_alerts` drops it from the queue and the watchdog is free to raise a fresh
//!    alert.
//!
//! `AlertManager::suppress_alerts` (the master-warning clear action) suppresses every
//! currently queued alert at once, regardless of severity or source.
#![allow(dead_code)]
use crate::hardware::sensor_manager::SensorManager;
use crate::alerts::watchdog::Watchdog;
use crate::alerts::alert::Alert;
use crate::graphics::ui_style::*;
use crate::graphics::context::GraphicsContext;

#[derive(Debug, Clone, Copy)]
pub enum Severity {
    Warning,
    Critical,
}

// UI style settings for new alerts
pub struct AlertStyle {
    pub font_path: String,
    pub font_size: f32,
    pub warning_color: (f32, f32, f32),
    pub critical_color: (f32, f32, f32),
    pub border_color: (f32, f32, f32),
    pub border_width: f32,
    pub margin: f32,
    pub corner_radius: f32,
    pub background_color: (f32, f32, f32), // Changed from 4 elements to 3
}

pub struct AlertManager {
    watchdog_id_counter: u32,       // Unique ID number to match watchdogs to alerts
    enabled: bool,
    watchdogs: Vec<(u32, Watchdog)>,
    alerts: Vec<(u32, Alert)>,
    alert_style: AlertStyle,
    sound_path: String,
}

impl AlertManager {
    /// Creates an empty alert manager (no watchdogs registered yet), loading alert
    /// display styling (colors, font, sound) from `ui_style`.
    pub fn new(enabled: bool, ui_style: &UIStyle) -> Self {
        Self {
            watchdog_id_counter: 0,
            enabled,
            watchdogs: Vec::new(),
            alerts: Vec::new(),
            alert_style: AlertStyle {
                font_path: ui_style.get_string(ALERT_FONT_PATH, DEFAULT_GLOBAL_FONT_PATH),
                font_size: ui_style.get_float(ALERT_FONT_SIZE, 32.0),
                warning_color: ui_style.get_color(ALERT_WARNING_COLOR, (1.0, 1.0, 0.0)),
                critical_color: ui_style.get_color(ALERT_CRITICAL_COLOR, (1.0, 0.0, 0.0)),
                border_color: ui_style.get_color(ALERT_BORDER_COLOR, (1.0, 1.0, 1.0)),
                border_width: ui_style.get_float(ALERT_BORDER_WIDTH, 4.0),
                margin: ui_style.get_float(ALERT_MARGIN, 8.0),
                corner_radius: ui_style.get_float(ALERT_CORNER_RADIUS, 8.0),
                background_color: ui_style.get_color(ALERT_BACKGROUND_COLOR, (0.0, 0.0, 0.0)),
            },
            sound_path: ui_style.get_string(ALERT_SOUND_PATH, ""),
        }
    }

    fn get_next_watchdog_id(&mut self) -> u32 {
        let id = self.watchdog_id_counter;
        self.watchdog_id_counter += 1;
        id
    }

    /// Enables or disables the alert manager as a whole. While disabled,
    /// `check_watchdogs` skips polling (no new alerts are raised) and `render_alerts`
    /// draws nothing and skips expiring alerts from the queue — timers keep running in
    /// the background regardless, so alerts may already be expired by the time
    /// rendering resumes.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Immediately suppresses every alert currently in the queue (see
    /// `Alert::suppress`), regardless of severity or which watchdog raised it. Used by
    /// the master-warning clear action.
    pub fn suppress_alerts(&mut self) {
        for alert in &mut self.alerts {
            alert.1.suppress();
        }
    }

    /// Registers a watchdog to be polled by `check_watchdogs`, assigning it a unique id
    /// used to match it to the alert it raises.
    pub fn add_watchdog(&mut self, watchdog: Watchdog) {
        let id = self.get_next_watchdog_id();
        self.watchdogs.push((id, watchdog));
    }

    /// Polls every registered watchdog once and raises a new `Alert` for each one whose
    /// condition just fired, unless that watchdog already has an alert in the queue
    /// (see the module-level lifecycle docs). No-op while the manager is disabled.
    pub fn check_watchdogs(&mut self, sensor_manager: &SensorManager) {
        if !self.enabled {
            return;
        }
        for (watchdog_id, watchdog) in &mut self.watchdogs {
            if watchdog.check(sensor_manager) {
                let already_active = self.alerts.iter().any(|(alert_id, _)| alert_id == watchdog_id);
                if already_active {
                    // Alert already active for this watchdog, skip adding a new one —
                    // but keep checking the remaining watchdogs.
                    continue;
                }
                log::info!("Watchdog: {:?} condition on {:?}", watchdog.severity(), watchdog.hw_input());
                self.alerts.push((*watchdog_id, Alert::new(
                    watchdog.message().clone(),
                    watchdog.severity(),
                    watchdog.alert_display_timeout(),
                    watchdog.alert_remove_timeout(),
                )));
            }
        }
    }

    /// Drops expired alerts from the queue, then draws every currently active alert,
    /// stacked vertically bottom-up (anchored to the bottom of the screen, growing
    /// upward as more alerts appear), with critical alerts sorted ahead of warnings.
    /// No-op while the manager is disabled.
    pub fn render_alerts(&mut self, context: &mut GraphicsContext) {
        if !self.enabled {
            return;
        }

        // Filter out expired alerts first
        self.alerts.retain(|alert| !alert.1.is_expired());

        if self.alerts.is_empty() {
            return;
        }

        // Copy active alerts to calculate layout properly, critical alerts first
        let mut active_alerts: Vec<&(u32, Alert)> = self.alerts
            .iter()
            .filter(|&(_, alert)| alert.is_active())
            .collect();
        active_alerts.sort_by_key(|(_, alert)| match alert.severity() {
            Severity::Critical => 0,
            Severity::Warning => 1,
        });

        let screen_width = context.width as f32;
        let screen_height = context.height as f32;
        let active_alert_count = active_alerts.len();

        if active_alert_count == 0 {
            return; // No active alerts to render
        }

        // Calculate text height for proper bounds sizing
        let text_height = match context.calculate_text_height_with_font(
            "Mg", // Sample text with ascenders and descenders to get maximum height
            1.0,
            &self.alert_style.font_path,
            self.alert_style.font_size as u32
        ) {
            Ok(height) => height,
            Err(_) => self.alert_style.font_size, // Fallback to font size
        };

        // Calculate text width for proper bounds sizing
        let mut max_text_width = 0.0;
        for alert in active_alerts.iter() {
            let width = context.calculate_text_width_with_font(
                &alert.1.message(),
                1.0,
                &self.alert_style.font_path,
                self.alert_style.font_size as u32
            );
            if let Ok(w) = width {
                if w - max_text_width > std::f32::EPSILON {
                    max_text_width = w;
                }
            }
        }

        // Calculate alert bounds height as: text_height * 2 + border_width + border_outer_margin
        let alert_height = text_height * 2.0 + self.alert_style.border_width + self.alert_style.margin;
        
        // Calculate total height needed for all alerts including spacing
        let total_alerts_height = (alert_height * active_alert_count as f32) + 
                                 (self.alert_style.margin * (active_alert_count - 1) as f32);
        
        // Anchor the alert stack to the bottom of the screen; it grows upward as
        // active_alert_count increases, so the bottom-most alert's position stays fixed.
        let x_offset = (screen_width - max_text_width - self.alert_style.margin) / 2.0;
        let start_y = screen_height - total_alerts_height - self.alert_style.margin;
        
        let mut y_offset = start_y;

        // Erase background
        let _ = context.fill_rect(
            x_offset - self.alert_style.margin,
            start_y - self.alert_style.margin,
            max_text_width + 2.0 * self.alert_style.margin,
            total_alerts_height + 2.0 * self.alert_style.margin,
            self.alert_style.background_color,
        );

        // Render each alert with calculated positioning; iterate in reverse so the
        // most severe (first in sorted order) lands in the bottom-most slot, closest
        // to the bottom-anchored edge of the stack.
        for alert in active_alerts.iter().rev() {
            let bounds = crate::indicators::indicator::IndicatorBounds {
                x: x_offset + self.alert_style.margin,
                y: y_offset + self.alert_style.margin,
                width: screen_width - 2.0 * self.alert_style.margin,
                height: alert_height,
            };

            if let Err(e) = alert.1.render(bounds, context, &self.alert_style) {
                log::error!("Error rendering alert \"{}\": {}", alert.1.message(), e);
            }

            y_offset += alert_height + self.alert_style.margin;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::ui_style::UIStyle;
    use crate::hardware::hw_providers::{HWAnalogProvider, HWInput};
    use crate::hardware::sensor_manager::SensorAnalogInputChain;
    use crate::hardware::sensor_value::ValueConstraints;
    use crate::hardware::sensors::GenericAnalogSensor;

    // Returns a fixed raw value regardless of input, so tests can control exactly what the
    // watchdog under test sees without depending on TestAnalogDataProvider's time-based pattern.
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
    // chain for `input`, reporting exactly `raw_value` with critical_high=90.
    fn manager_with_fixed_value(input: HWInput, raw_value: u16) -> SensorManager {
        let mut manager = SensorManager::new();
        let chain = SensorAnalogInputChain::new(
            Box::new(FixedAnalogDataProvider { input, raw_value }),
            vec![],
            Box::new(GenericAnalogSensor::new(
                "alert_manager_test".to_string(), "Alert Manager Test".to_string(), "".to_string(),
                ValueConstraints::analog_with_thresholds(0.0, 100.0, None, None, None, Some(90.0)),
                1.0,
            )),
        );
        manager.add_analog_sensor_chain(chain);
        manager.read_all_sensors().expect("fixed provider read should never fail");
        manager
    }

    fn triggering_watchdog() -> Watchdog {
        Watchdog::new(HWInput::HwEngineCoolantTemp, "overheating".to_string(), Severity::Critical, None, None, None)
    }

    #[test]
    fn test_disabled_manager_does_not_raise_alerts() {
        let ui_style = UIStyle::new();
        let mut manager = AlertManager::new(false, &ui_style);
        manager.add_watchdog(triggering_watchdog());

        let sensor_manager = manager_with_fixed_value(HWInput::HwEngineCoolantTemp, 95);
        manager.check_watchdogs(&sensor_manager);

        assert!(manager.alerts.is_empty(), "a disabled manager must not poll watchdogs at all");
    }

    #[test]
    fn test_check_watchdogs_raises_alert_for_triggering_condition() {
        let ui_style = UIStyle::new();
        let mut manager = AlertManager::new(true, &ui_style);
        manager.add_watchdog(triggering_watchdog());

        let sensor_manager = manager_with_fixed_value(HWInput::HwEngineCoolantTemp, 95);
        manager.check_watchdogs(&sensor_manager);

        assert_eq!(manager.alerts.len(), 1);
    }

    #[test]
    fn test_check_watchdogs_does_not_raise_duplicate_while_alert_active() {
        let ui_style = UIStyle::new();
        let mut manager = AlertManager::new(true, &ui_style);
        manager.add_watchdog(triggering_watchdog());

        let sensor_manager = manager_with_fixed_value(HWInput::HwEngineCoolantTemp, 95);
        manager.check_watchdogs(&sensor_manager);
        manager.check_watchdogs(&sensor_manager);
        manager.check_watchdogs(&sensor_manager);

        assert_eq!(manager.alerts.len(), 1, "a watchdog with an already-queued alert must not raise a second one");
    }

    #[test]
    fn test_check_watchdogs_is_noop_when_condition_not_met() {
        let ui_style = UIStyle::new();
        let mut manager = AlertManager::new(true, &ui_style);
        manager.add_watchdog(triggering_watchdog());

        let sensor_manager = manager_with_fixed_value(HWInput::HwEngineCoolantTemp, 10); // well below critical_high
        manager.check_watchdogs(&sensor_manager);

        assert!(manager.alerts.is_empty());
    }

    #[test]
    fn test_suppress_alerts_marks_every_queued_alert_inactive() {
        let ui_style = UIStyle::new();
        let mut manager = AlertManager::new(true, &ui_style);
        manager.add_watchdog(triggering_watchdog());

        let sensor_manager = manager_with_fixed_value(HWInput::HwEngineCoolantTemp, 95);
        manager.check_watchdogs(&sensor_manager);
        assert_eq!(manager.alerts.len(), 1);
        assert!(manager.alerts[0].1.is_active(), "sanity check: alert starts out active");

        manager.suppress_alerts();

        assert!(manager.alerts.iter().all(|(_, alert)| !alert.is_active()),
               "suppress_alerts must force every queued alert inactive regardless of source");
    }
}