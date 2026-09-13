//! Owns the set of hardware watchdogs and the alerts they've raised, and renders the
//! currently active ones to screen.
//!
//! ## Alert lifecycle
//! 1. Each cycle, `AlertManager::check_watchdogs` first purges any expired alert from the
//!    queue (see `Alert::is_expired`), then polls every registered `Watchdog`. The first
//!    time a watchdog's condition fires, a matching `Alert` is created and added to the
//!    queue, tagged with the watchdog's id so at most one alert exists per watchdog at a
//!    time. Purging before polling — rather than after, as part of rendering — matters
//!    for a watchdog configured with a short `alert_display_timeout` and a near-zero
//!    `alert_remove_timeout`: a still-triggering condition can raise a fresh alert in the
//!    very same cycle its previous alert expires, so `render_alerts` never sees a gap
//!    frame with nothing to draw ("self-refreshing" alerts, used e.g. for an always-on
//!    "mode active" indicator).
//! 2. `AlertManager::render_alerts` draws every alert that is currently active (see
//!    `Alert`'s docs for what "active" means and how `display_timeout` governs it),
//!    stacked and centered on screen.
//! 3. Once an alert stops being active (its `display_timeout` elapsed, or it was
//!    suppressed), it lingers in the queue — still blocking its watchdog from raising a
//!    duplicate — until `Alert::is_expired` returns true, at which point the next
//!    `check_watchdogs` call drops it from the queue and the watchdog is free to raise a
//!    fresh alert.
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
                font_path: ui_style.get_string(StyleKey::AlertFontPath),
                font_size: ui_style.get_float(StyleKey::AlertFontSize),
                warning_color: ui_style.get_color(StyleKey::AlertWarningColor),
                critical_color: ui_style.get_color(StyleKey::AlertCriticalColor),
                border_color: ui_style.get_color(StyleKey::AlertBorderColor),
                border_width: ui_style.get_float(StyleKey::AlertBorderWidth),
                margin: ui_style.get_float(StyleKey::AlertMargin),
                corner_radius: ui_style.get_float(StyleKey::AlertCornerRadius),
                background_color: ui_style.get_color(StyleKey::AlertBackgroundColor),
            },
            sound_path: ui_style.get_string(StyleKey::AlertSoundPath),
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

    /// True if at least one currently queued alert is active (i.e. would currently be
    /// rendered by `render_alerts`). Drives the master warning LED blink.
    pub fn has_active_alerts(&self) -> bool {
        self.alerts.iter().any(|(_, alert)| alert.is_active())
    }

    /// Registers a watchdog to be polled by `check_watchdogs`, assigning it a unique id
    /// used to match it to the alert it raises.
    pub fn add_watchdog(&mut self, watchdog: Watchdog) {
        let id = self.get_next_watchdog_id();
        self.watchdogs.push((id, watchdog));
    }

    /// Purges expired alerts, then polls every registered watchdog once and raises a new
    /// `Alert` for each one whose condition just fired, unless that watchdog already has
    /// an alert in the queue (see the module-level lifecycle docs — purging first is what
    /// lets a self-refreshing alert re-fire in the same cycle it expires). No-op while
    /// the manager is disabled.
    ///
    /// A watchdog whose alert was just purged *in this same call* is a self-refresh
    /// continuation, not a new occurrence — its "condition on" log line is skipped (unlike
    /// every other watchdog in this codebase, which logs once and then goes quiet for
    /// minutes, a self-refreshing one would otherwise log every ~1s indefinitely). Confirmed
    /// on-device that this log line's synchronous file+stdout write was exactly what caused
    /// a real, reproducible single-frame miss (~33ms gap instead of ~17ms) once per refresh
    /// cycle — visible on screen as a brief dim/see-through flash of the alert, since that
    /// frame's already-drawn page content showed through where the alert hadn't redrawn yet.
    pub fn check_watchdogs(&mut self, sensor_manager: &SensorManager) {
        if !self.enabled {
            return;
        }
        let mut just_purged: Vec<u32> = Vec::new();
        self.alerts.retain(|(id, alert)| {
            let expired = alert.is_expired();
            if expired {
                just_purged.push(*id);
            }
            !expired
        });
        for (watchdog_id, watchdog) in &mut self.watchdogs {
            if watchdog.check(sensor_manager) {
                let already_active = self.alerts.iter().any(|(alert_id, _)| alert_id == watchdog_id);
                if already_active {
                    // Alert already active for this watchdog, skip adding a new one —
                    // but keep checking the remaining watchdogs.
                    continue;
                }
                if !just_purged.contains(watchdog_id) {
                    log::info!("Watchdog: {:?} condition on {:?}", watchdog.severity(), watchdog.hw_input());
                }
                self.alerts.push((*watchdog_id, Alert::new(
                    watchdog.message().clone(),
                    watchdog.severity(),
                    watchdog.alert_display_timeout(),
                    watchdog.alert_remove_timeout(),
                )));
            }
        }
    }

    /// Draws every currently active alert, stacked vertically bottom-up (anchored to the
    /// bottom of the screen, growing upward as more alerts appear), with critical alerts
    /// sorted ahead of warnings. Expired alerts are purged by `check_watchdogs`, not here
    /// (see the module-level lifecycle docs). No-op while the manager is disabled.
    pub fn render_alerts(&mut self, context: &mut GraphicsContext) {
        if !self.enabled {
            return;
        }

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
                alert.1.message(),
                1.0,
                &self.alert_style.font_path,
                self.alert_style.font_size as u32
            );
            if let Ok(w) = width {
                if w - max_text_width > f32::EPSILON {
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

        // Erase background. Surfaced rather than silently discarded (as it used to be) --
        // a failed erase would still let the text below draw, but over whatever
        // render_current_page() already left on screen (e.g. gauge needles), which would
        // look like a semi-transparent/see-through alert background.
        if let Err(e) = context.fill_rect(
            x_offset - self.alert_style.margin,
            start_y - self.alert_style.margin,
            max_text_width + 2.0 * self.alert_style.margin,
            total_alerts_height + 2.0 * self.alert_style.margin,
            self.alert_style.background_color,
        ) {
            log::error!("Error erasing alert background: {}", e);
        }

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
        let ui_style = UIStyle::from_file(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui_style.json"))
            .expect("repo ui_style.json should load and validate");
        let mut manager = AlertManager::new(false, &ui_style);
        manager.add_watchdog(triggering_watchdog());

        let sensor_manager = manager_with_fixed_value(HWInput::HwEngineCoolantTemp, 95);
        manager.check_watchdogs(&sensor_manager);

        assert!(manager.alerts.is_empty(), "a disabled manager must not poll watchdogs at all");
    }

    #[test]
    fn test_check_watchdogs_raises_alert_for_triggering_condition() {
        let ui_style = UIStyle::from_file(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui_style.json"))
            .expect("repo ui_style.json should load and validate");
        let mut manager = AlertManager::new(true, &ui_style);
        manager.add_watchdog(triggering_watchdog());

        let sensor_manager = manager_with_fixed_value(HWInput::HwEngineCoolantTemp, 95);
        manager.check_watchdogs(&sensor_manager);

        assert_eq!(manager.alerts.len(), 1);
    }

    #[test]
    fn test_check_watchdogs_does_not_raise_duplicate_while_alert_active() {
        let ui_style = UIStyle::from_file(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui_style.json"))
            .expect("repo ui_style.json should load and validate");
        let mut manager = AlertManager::new(true, &ui_style);
        manager.add_watchdog(triggering_watchdog());

        let sensor_manager = manager_with_fixed_value(HWInput::HwEngineCoolantTemp, 95);
        manager.check_watchdogs(&sensor_manager);
        manager.check_watchdogs(&sensor_manager);
        manager.check_watchdogs(&sensor_manager);

        assert_eq!(manager.alerts.len(), 1, "a watchdog with an already-queued alert must not raise a second one");
    }

    #[test]
    fn test_check_watchdogs_refreshes_self_expiring_alert_in_the_same_cycle_it_expires() {
        // display_timeout short + remove_timeout ~0 is the "self-refreshing" pattern used
        // for an always-on "mode active" indicator: as long as the condition persists, a
        // just-expired alert must be replaced by a fresh (active) one within the same
        // check_watchdogs call, not a frame later (that gap would show as a blink).
        let ui_style = UIStyle::from_file(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui_style.json"))
            .expect("repo ui_style.json should load and validate");
        let mut manager = AlertManager::new(true, &ui_style);
        // manager_with_fixed_value below only sets critical_high (no warning_high), so the
        // watchdog must be Critical severity for its condition to actually trigger — the
        // severity itself is incidental to what this test is proving.
        manager.add_watchdog(Watchdog::new(
            HWInput::HwEngineCoolantTemp, "sticky".to_string(), Severity::Critical,
            Some(std::time::Duration::from_millis(10)), Some(std::time::Duration::ZERO), None,
        ));

        let sensor_manager = manager_with_fixed_value(HWInput::HwEngineCoolantTemp, 95);
        manager.check_watchdogs(&sensor_manager);
        assert_eq!(manager.alerts.len(), 1);
        assert!(manager.alerts[0].1.is_active(), "sanity check: freshly raised alert is active");

        std::thread::sleep(std::time::Duration::from_millis(20)); // past display_timeout

        manager.check_watchdogs(&sensor_manager);
        assert_eq!(manager.alerts.len(), 1, "condition still holds, so exactly one alert stays queued");
        assert!(manager.alerts[0].1.is_active(),
            "a persisting condition must raise a fresh alert in the same call its old one expires");
    }

    #[test]
    fn test_check_watchdogs_is_noop_when_condition_not_met() {
        let ui_style = UIStyle::from_file(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui_style.json"))
            .expect("repo ui_style.json should load and validate");
        let mut manager = AlertManager::new(true, &ui_style);
        manager.add_watchdog(triggering_watchdog());

        let sensor_manager = manager_with_fixed_value(HWInput::HwEngineCoolantTemp, 10); // well below critical_high
        manager.check_watchdogs(&sensor_manager);

        assert!(manager.alerts.is_empty());
    }

    #[test]
    fn test_suppress_alerts_marks_every_queued_alert_inactive() {
        let ui_style = UIStyle::from_file(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui_style.json"))
            .expect("repo ui_style.json should load and validate");
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