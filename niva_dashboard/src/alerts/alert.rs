#![allow(dead_code)]
use crate::alerts::alert_manager::{Severity, AlertStyle};
use crate::graphics::context::GraphicsContext;
use crate::indicators::indicator::IndicatorBounds;

pub struct Alert {
    message: String,
    severity: Severity,
    display_timeout: Option<std::time::Duration>,
    remove_timeout: Option<std::time::Duration>,
    creation_time: std::time::Instant,
}

impl Alert {
    pub fn new(message: String, severity: Severity, display_timeout: Option<std::time::Duration>, remove_timeout: Option<std::time::Duration>) -> Self {
        Self {
            message,
            severity,
            display_timeout,
            remove_timeout,
            creation_time: std::time::Instant::now(),
        }
    }

    pub fn render(&self, bounds: IndicatorBounds, context: &mut GraphicsContext,
                  alert_style: &AlertStyle) -> Result<(), String> {

        let text_color = match self.severity {
            Severity::Warning => alert_style.warning_color,
            Severity::Critical => alert_style.critical_color,
        };

        context.render_text_with_font(
            &self.message,
            bounds.x,
            bounds.y,
            1.0,
            text_color,
            &alert_style.font_path,
            alert_style.font_size as u32,
        )?;
        Ok(())
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn severity(&self) -> Severity {
        self.severity
    }

    pub fn suppress(&mut self) {
        self.display_timeout = Some(std::time::Duration::ZERO);
        self.creation_time = std::time::Instant::now();     // Reset creation time for remove_timeout
    }

    pub fn is_active(&self) -> bool {
        match self.display_timeout {
            None => true, // Always active if no timeout set
            Some(timeout) => self.creation_time.elapsed() < timeout,
        }
    }

    // Return true if the alert has expired based on inactivity and remove_timeout
    // and should be removed from queue.
    pub fn is_expired(&self) -> bool {
        match self.remove_timeout {
            None => false, // Never expires if no timeout set.
            Some(_) if self.is_active() => false,
            Some(timeout) => self.creation_time.elapsed() >= timeout,
        }
    }
}