use crate::alerts::alert_manager::Severity;
use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::UIStyle;
use crate::indicators::indicator::IndicatorBounds;

pub struct Alert {
    message: String,
    severity: Severity,
    timeout_ms: u32,
}

impl Alert {
    pub fn new(message: String, severity: Severity, timeout_ms: u32) -> Self {
        Self {
            message,
            severity,
            timeout_ms,
        }
    }

    pub fn render(&self, bounds: IndicatorBounds, context: &mut GraphicsContext,
                  text_color: (f32, f32, f32), font_path: &str, font_size: f32) -> Result<(), String> {
        context.render_text_with_font(
            &self.message,
            bounds.x,
            bounds.y,
            1.0,
            text_color,
            font_path,
            font_size as u32,
        )?;
        Ok(())
    }

    pub fn severity(&self) -> Severity {
        self.severity
    }
}