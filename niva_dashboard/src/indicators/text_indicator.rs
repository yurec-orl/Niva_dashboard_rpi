use crate::indicators::indicator::{Indicator, SensorValue, ValueData, IndicatorBounds};
use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::{UIStyle, TEXT_PRIMARY_FONT, TEXT_PRIMARY_FONT_SIZE, TEXT_PRIMARY_COLOR, TEXT_WARNING_COLOR, TEXT_ERROR_COLOR};

/// Simple text indicator that displays sensor values as formatted text
pub struct TextIndicator {
    /// Format precision for floating point values
    precision: usize,
    /// Whether to show the unit after the value
    show_unit: bool,
    /// Whether to show the label before the value
    show_label: bool,
    /// Text alignment within bounds
    alignment: TextAlignment,
}

#[derive(Debug, Clone, Copy)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
}

impl TextIndicator {
    /// Create a new text indicator with default settings
    pub fn new() -> Self {
        Self {
            precision: 1,
            show_unit: true,
            show_label: false,
            alignment: TextAlignment::Center,
        }
    }
    
    /// Create a text indicator with custom settings
    pub fn with_config(
        precision: usize,
        show_unit: bool,
        show_label: bool,
        alignment: TextAlignment,
    ) -> Self {
        Self {
            precision,
            show_unit,
            show_label,
            alignment,
        }
    }
    
    /// Format the sensor value as a display string
    fn format_value(&self, value: &SensorValue) -> String {
        let value_str = match value.value {
            ValueData::Digital(b) => {
                if b { "ON".to_string() } else { "OFF".to_string() }
            }
            ValueData::Analog(v) => {
                format!("{:.prec$}", v, prec = self.precision)
            }
            ValueData::Percentage(p) => {
                format!("{:.prec$}%", p, prec = self.precision)
            }
            ValueData::Integer(i) => {
                format!("{}", i)
            }
        };
        
        let mut result = String::new();
        
        // Add label if requested
        if self.show_label && !value.metadata.label.is_empty() {
            result.push_str(&value.metadata.label);
            result.push_str(": ");
        }
        
        // Add the value
        result.push_str(&value_str);
        
        // Add unit if requested and available
        if self.show_unit && !value.metadata.unit.is_empty() {
            // Don't add unit for percentages (already included) or digital values
            if !matches!(value.value, ValueData::Percentage(_) | ValueData::Digital(_)) {
                result.push(' ');
                result.push_str(&value.metadata.unit);
            }
        }
        
        result
    }
    
    /// Get text color based on value status
    fn get_text_color(&self, value: &SensorValue) -> &'static str {
        if value.is_critical() {
            TEXT_ERROR_COLOR
        } else if value.is_warning() {
            TEXT_WARNING_COLOR
        } else {
            TEXT_PRIMARY_COLOR
        }
    }
    
    /// Calculate text position based on alignment
    fn calculate_text_position(&self, bounds: IndicatorBounds, text_width: f32) -> (f32, f32) {
        let x = match self.alignment {
            TextAlignment::Left => bounds.x,
            TextAlignment::Center => bounds.x + (bounds.width - text_width) / 2.0,
            TextAlignment::Right => bounds.x + bounds.width - text_width,
        };
        
        // Vertically center the text
        let y = bounds.y + bounds.height / 2.0;
        
        (x, y)
    }
}

impl Default for TextIndicator {
    fn default() -> Self {
        Self::new()
    }
}

impl Indicator for TextIndicator {
    fn render(
        &mut self,
        value: &SensorValue,
        bounds: IndicatorBounds,
        style: &UIStyle,
        context: &mut GraphicsContext,
    ) -> Result<(), String> {
        // Format the text to display
        let display_text = self.format_value(value);
        
        // Get style parameters
        let font_path = style.get_string(TEXT_PRIMARY_FONT, "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf");
        let font_size = style.get_integer(TEXT_PRIMARY_FONT_SIZE, 24) as u32;
        let text_color = style.get_color(self.get_text_color(value), (1.0, 1.0, 1.0)); // Default to white
        
        // Calculate text dimensions
        let scale = 1.0; // Default scale factor
        let text_width = context.calculate_text_width_with_font(
            &display_text,
            scale,
            &font_path,
            font_size,
        )?;
        
        // Calculate position based on alignment
        let (x, y) = self.calculate_text_position(bounds, text_width);
        
        // Render the text
        context.render_text_with_font(
            &display_text,
            x,
            y,
            scale,
            text_color,
            &font_path,
            font_size,
        )?;
        
        Ok(())
    }
    
    fn preferred_size(&self, style: &UIStyle) -> (f32, f32) {
        let font_size = style.get_integer(TEXT_PRIMARY_FONT_SIZE, 24) as f32;
        
        // Estimate size based on typical text content
        // This is a rough estimate - actual size depends on the value being displayed
        let estimated_width = font_size * 8.0; // Approximate width for "123.4 °C"
        let estimated_height = font_size * 1.5; // Font size + some padding
        
        (estimated_width, estimated_height)
    }
    
    fn indicator_type(&self) -> &'static str {
        "TextIndicator"
    }
    
    fn supports_value_type(&self, value: &ValueData) -> bool {
        // Text indicator can display any value type
        match value {
            ValueData::Digital(_) => true,
            ValueData::Analog(_) => true,
            ValueData::Percentage(_) => true,
            ValueData::Integer(_) => true,
        }
    }
}