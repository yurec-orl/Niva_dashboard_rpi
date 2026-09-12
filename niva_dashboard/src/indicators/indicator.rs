#![allow(dead_code)]
use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::UIStyle;
use crate::hardware::sensor_value::{SensorValue, ValueData};
use crate::indicators::decorator::Decorator;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Position and size information for indicator rendering
#[derive(Debug, Clone, Copy)]
pub struct IndicatorBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl IndicatorBounds {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }
    
    /// Get center point of the bounds
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

pub struct IndicatorBase {
    pub decorators: Vec<Box<dyn Decorator>>,
}

impl IndicatorBase {
    pub fn new() -> Self {
        Self {
            decorators: Vec::new(),
        }
    }

    pub fn render_decorators(
        &self,
        bounds: IndicatorBounds,
        style: &UIStyle,
        context: &mut GraphicsContext,
    ) -> Result<(), String> {
        for decorator in &self.decorators {
            decorator.render(bounds, style, context)?;
        }
        Ok(())
    }
}

/// Main indicator trait for rendering various dashboard indicators
pub trait Indicator {
    fn with_decorators(self, decorators: Vec<Box<dyn Decorator>>) -> Self where Self: Sized;

    /// Render the indicator with the given value, bounds, style and graphics context
    /// 
    /// # Parameters
    /// - `value`: The sensor value with its constraints and metadata
    /// - `bounds`: Position and size constraints for the indicator
    /// - `style`: UI styling parameters (colors, fonts, sizes, etc.)
    /// - `context`: Graphics context for OpenGL rendering operations
    fn render(&self, 
              value: &SensorValue, 
              bounds: IndicatorBounds, 
              style: &UIStyle, 
              context: &mut GraphicsContext) -> Result<(), String>;
    /// Get indicator type name for debugging and configuration
    fn indicator_type(&self) -> &'static str;
    
    /// Check if indicator can handle the given value type efficiently
    fn supports_value_type(&self, _value: &ValueData) -> bool {
        // Individual indicators can override for optimization
        false
    }
}

/// Whether a blink with the given full period is currently in its "on" half, sampled from
/// wall-clock time rather than tracked per-indicator state -- `Indicator::render()` gets no
/// time delta, and indicators are stateless (shared across frames), so there's nowhere to
/// store a per-instance timer.
fn blink_phase_on(period: Duration) -> bool {
    let half_period_ms = (period.as_millis() / 2).max(1);
    let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    (now_ms / half_period_ms) % 2 == 0
}

/// True during the "on" half of a 2Hz blink (250ms on, 250ms off) -- the cadence used by
/// every fault visual below, and matching the master warning LED's blink rate.
pub fn fault_blink_on() -> bool {
    blink_phase_on(Duration::from_millis(500))
}

/// Draws a blinking red X centered at (cx, cy), each arm spanning `half_diagonal` from
/// center to tip. Indicators show this in place of a needle/fill when their paired sensor
/// has no reading, so a fault can never be mistaken for a real zero/min value (issue #28).
/// Red is fixed, not themeable via UIStyle -- a fault glyph shouldn't be able to blend into
/// a color scheme.
pub fn render_fault_x(context: &mut GraphicsContext, cx: f32, cy: f32, half_diagonal: f32) -> Result<(), String> {
    if !fault_blink_on() {
        return Ok(());
    }
    let color = (1.0, 0.0, 0.0);
    let thickness = half_diagonal * 0.24;
    context.render_line((cx - half_diagonal, cy - half_diagonal), (cx + half_diagonal, cy + half_diagonal), color, thickness)?;
    context.render_line((cx - half_diagonal, cy + half_diagonal), (cx + half_diagonal, cy - half_diagonal), color, thickness)?;
    Ok(())
}