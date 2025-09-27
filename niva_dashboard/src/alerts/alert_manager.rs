use crate::hardware::sensor_manager::SensorManager;
use crate::alerts::watchdog::Watchdog;
use crate::alerts::alert::Alert;
use crate::graphics::ui_style::*;

// AlertManager is responsible for managing alerts and watchdogs.
// Watchdogs are used to monitor hardware inputs and trigger alerts when certain conditions are met.
// Alerts are displayed on screen and can have different severities and timeouts.

#[derive(Debug, Clone, Copy)]
pub enum Severity {
    Warning,
    Critical,
}

pub struct AlertManager {
    enabled: bool,
    watchdogs: Vec<Watchdog>,
    alerts: Vec<Alert>,
    font_path: String,          // ui style settings for new alerts
    font_size: f32,
    warning_color: (f32, f32, f32),
    critical_color: (f32, f32, f32),
    border_color: (f32, f32, f32),
    border_width: f32,
    border_outer_margin: f32,
    sound_path: String,
}

impl AlertManager {
    pub fn new(enabled: bool, ui_style: &UIStyle) -> Self {
        Self {
            enabled,
            watchdogs: Vec::new(),
            alerts: Vec::new(),
            font_path: ui_style.get_string(ALERT_FONT_PATH, DEFAULT_GLOBAL_FONT_PATH),
            font_size: ui_style.get_float(ALERT_FONT_SIZE, 32.0),
            warning_color: ui_style.get_color(ALERT_WARNING_COLOR, (1.0, 1.0, 0.0)),
            critical_color: ui_style.get_color(ALERT_CRITICAL_COLOR, (1.0, 0.0, 0.0)),
            border_color: ui_style.get_color(ALERT_BORDER_COLOR, (1.0, 1.0, 1.0)),
            border_width: ui_style.get_float(ALERT_BORDER_WIDTH, 4.0),
            border_outer_margin: ui_style.get_float(ALERT_BORDER_OUTER_MARGIN, 8.0),
            sound_path: ui_style.get_string(ALERT_SOUND_PATH, ""),
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn add_watchdog(&mut self, watchdog: Watchdog) {
        self.watchdogs.push(watchdog);
    }

    pub fn check_watchdogs(&mut self, sensor_manager: &SensorManager) {
        if !self.enabled {
            return;
        }
        for watchdog in &self.watchdogs {
            if watchdog.check(sensor_manager) {
                self.alerts.push(Alert::new(
                    watchdog.message().clone(),
                    watchdog.severity(),
                    watchdog.timeout_ms(),
                ));
            }
        }
    }

    pub fn render_alerts(&mut self, context: &mut crate::graphics::context::GraphicsContext) {
        if !self.enabled {
            return;
        }
        let screen_width = context.width as f32;
        let screen_height = context.height as f32;

        let mut y_offset = self.border_outer_margin;

        self.alerts.retain(|alert| {
            let bounds = crate::indicators::indicator::IndicatorBounds {
                x: self.border_outer_margin,
                y: y_offset,
                width: screen_width - 2.0 * self.border_outer_margin,
                height: self.font_size + 2.0 * self.border_outer_margin,
            };
            let text_color = match alert.severity() {
                Severity::Warning => self.warning_color,
                Severity::Critical => self.critical_color,
            };
            if let Err(e) = alert.render(bounds, context, text_color, &self.font_path, self.font_size) {
                eprintln!("Error rendering alert: {}", e);
            }
            y_offset += bounds.height + self.border_outer_margin;
            // Keep the alert if it has not timed out
            true // Placeholder: Implement timeout logic if needed
        });
    }
}