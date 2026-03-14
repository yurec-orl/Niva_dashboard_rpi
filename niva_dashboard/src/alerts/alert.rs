use crate::alerts::alert_manager::{Severity, AlertStyle};
use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::UIStyle;
use crate::indicators::indicator::IndicatorBounds;

pub struct Alert {
    message: String,
    severity: Severity,
    display_timeout_ms: Option<u32>,
    remove_timeout_ms: Option<u32>,
    creation_time: std::time::Instant,
}

impl Alert {
    pub fn new(message: String, severity: Severity, display_timeout_ms: Option<u32>, remove_timeout_ms: Option<u32>) -> Self {
        Self {
            message,
            severity,
            display_timeout_ms,
            remove_timeout_ms,
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
        self.display_timeout_ms = Some(0);
    }

    pub fn is_active(&self) -> bool {
        if self.display_timeout_ms.is_none() {
            return true; // Always active if no timeout set
        }
        self.creation_time.elapsed().as_millis() < self.display_timeout_ms.unwrap() as u128
    }

    // Return true if the alert has expired based on remove_timeout_ms
    // and should be removed from queue.
    pub fn is_expired(&self) -> bool {
        if self.remove_timeout_ms.is_none() {
            return false; // Never expires if no timeout set
        }
        self.creation_time.elapsed().as_millis() >= self.remove_timeout_ms.unwrap() as u128
    }
}