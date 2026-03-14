use crate::hardware::sensor_manager::SensorManager;
use crate::alerts::watchdog::Watchdog;
use crate::alerts::alert::Alert;
use crate::graphics::ui_style::*;
use crate::graphics::context::GraphicsContext;
use std::collections::HashMap;

// AlertManager is responsible for managing alerts and watchdogs.
// Watchdogs are used to monitor hardware inputs and trigger alerts when certain conditions are met.
// Alerts are displayed on screen and can have different severities and timeouts.
// Each watchdog can produce only one alert with a fixed message and severity.
// For any watchdog, there can be only one active alert at a time.

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

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn suppress_alerts(&mut self) {
        for alert in &mut self.alerts {
            alert.1.suppress();
        }
    }

    pub fn add_watchdog(&mut self, watchdog: Watchdog) {
        let id = self.get_next_watchdog_id();
        self.watchdogs.push((id, watchdog));
    }

    pub fn check_watchdogs(&mut self, sensor_manager: &SensorManager) {
        if !self.enabled {
            return;
        }
        for (watchdog_id, watchdog) in &mut self.watchdogs {
            if watchdog.check(sensor_manager) {
                for (alert_id, alert) in &self.alerts {
                    if alert_id == watchdog_id {
                        // Alert already active, skip adding a new one
                        return;
                    }
                }
                print!("Watchdog: {:?} condition on {:?}\r\n", watchdog.severity(), watchdog.hw_input());
                self.alerts.push((*watchdog_id, Alert::new(
                    watchdog.message().clone(),
                    watchdog.severity(),
                    watchdog.alert_display_timeout_ms(),
                    watchdog.alert_remove_timeout_ms(),
                )));
            }
        }
    }

    pub fn render_alerts(&mut self, context: &mut GraphicsContext) {
        if !self.enabled {
            return;
        }
        
        // Filter out expired alerts first
        self.alerts.retain(|alert| !alert.1.is_expired());
        
        if self.alerts.is_empty() {
            return;
        }

        // Copy active alerts to calculate layout properly
        let active_alerts: Vec<&(u32, Alert)> = self.alerts
            .iter()
            .filter(|&(_, alert)| alert.is_active())
            .collect();

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
        
        // Calculate starting Y coordinate to center alerts vertically on screen
        let x_offset = (screen_width - max_text_width - self.alert_style.margin) / 2.0;
        let start_y = (screen_height - total_alerts_height) / 2.0;
        
        let mut y_offset = start_y;

        // Erase background
        context.fill_rect(
            x_offset - self.alert_style.margin,
            start_y - self.alert_style.margin,
            max_text_width + 2.0 * self.alert_style.margin,
            total_alerts_height + 2.0 * self.alert_style.margin,
            self.alert_style.background_color,
        );

        // Render each alert with calculated positioning
        for alert in active_alerts.iter() {
            let bounds = crate::indicators::indicator::IndicatorBounds {
                x: x_offset + self.alert_style.margin,
                y: y_offset + self.alert_style.margin,
                width: screen_width - 2.0 * self.alert_style.margin,
                height: alert_height,
            };

            if let Err(e) = alert.1.render(bounds, context, &self.alert_style) {
                eprintln!("Error rendering alert \"{}\": {}", alert.1.message(), e);
            }

            y_offset += alert_height + self.alert_style.margin;
        }
    }
}