use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::UIStyle;
use crate::hardware::sensor_value::{SensorValue, ValueData};
use crate::indicators::decorator::Decorator;

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
    fn supports_value_type(&self, value: &ValueData) -> bool {
        // Individual indicators can override for optimization
        false
    }
}