use crate::indicators::indicator::{Indicator, IndicatorBounds};
use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::{UIStyle, TEXT_PRIMARY_FONT, TEXT_PRIMARY_FONT_SIZE, TEXT_PRIMARY_COLOR, TEXT_WARNING_COLOR, TEXT_ERROR_COLOR};
use crate::hardware::sensor_value::{SensorValue, ValueData};

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
    
    /// Format the sensor value as a display string (without label)
    fn format_value(&self, value: &SensorValue) -> String {
        let value_str = match value.value {
            ValueData::Empty => "N/A".to_string(),
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
        
        let mut result = value_str;
        
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
    
    /// Get the label text
    fn get_label(&self, value: &SensorValue) -> String {
        if self.show_label && !value.metadata.label.is_empty() {
            value.metadata.label.clone()
        } else {
            String::new()
        }
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
    
    /// Calculate text position for label and value (label above, value below, both centered)
    fn calculate_text_positions(
        &self, 
        bounds: IndicatorBounds, 
        label_width: f32, 
        value_width: f32,
        font_height: f32
    ) -> ((f32, f32), (f32, f32)) {
        // Calculate x positions (centered)
        let label_x = bounds.x + (bounds.width - label_width) / 2.0;
        let value_x = bounds.x + (bounds.width - value_width) / 2.0;
        
        // Calculate y positions (label in upper half, value in lower half)
        let center_y = bounds.y + bounds.height / 2.0;
        let spacing = font_height * 0.2; // Small spacing between label and value
        
        let label_y = center_y - spacing / 2.0 - font_height / 2.0;
        let value_y = center_y + spacing / 2.0 + font_height / 2.0;
        
        ((label_x, label_y), (value_x, value_y))
    }
}

impl Default for TextIndicator {
    fn default() -> Self {
        Self::new()
    }
}

impl Indicator for TextIndicator {
    fn render(
        &self,
        value: &SensorValue,
        bounds: IndicatorBounds,
        style: &UIStyle,
        context: &mut GraphicsContext,
    ) -> Result<(), String> {
        // Get label and value texts
        let label_text = self.get_label(value);
        let value_text = self.format_value(value);
        
        // Get style parameters
        let font_path = style.get_string(TEXT_PRIMARY_FONT, "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf");
        let font_size = style.get_integer(TEXT_PRIMARY_FONT_SIZE, 24) as u32;
        let text_color = style.get_color(self.get_text_color(value), (1.0, 1.0, 1.0)); // Default to white
        
        let scale = 1.0; // Default scale factor
        
        // Calculate text dimensions
        let label_width = if !label_text.is_empty() {
            context.calculate_text_width_with_font(
                &label_text,
                scale,
                &font_path,
                font_size,
            )?
        } else {
            0.0
        };
        
        let value_width = context.calculate_text_width_with_font(
            &value_text,
            scale,
            &font_path,
            font_size,
        )?;
        
        // Get font height for positioning
        let font_height = context.get_line_height_with_font(scale, &font_path, font_size)?;
        
        // Calculate positions for both texts
        let ((label_x, label_y), (value_x, value_y)) = self.calculate_text_positions(
            bounds, 
            label_width, 
            value_width, 
            font_height
        );
        
        // Render label if present
        if !label_text.is_empty() {
            context.render_text_with_font(
                &label_text,
                label_x,
                label_y,
                scale,
                text_color,
                &font_path,
                font_size,
            )?;
        }
        
        // Render value
        context.render_text_with_font(
            &value_text,
            value_x,
            value_y,
            scale,
            text_color,
            &font_path,
            font_size,
        )?;
        
        Ok(())
    }
    
    fn indicator_type(&self) -> &'static str {
        "TextIndicator"
    }
    
    fn supports_value_type(&self, value: &ValueData) -> bool {
        // Text indicator can display any value type
        match value {
            ValueData::Empty => true,       // Could be useful for "n/a" or static labels
            ValueData::Digital(_) => true,
            ValueData::Analog(_) => true,
            ValueData::Percentage(_) => true,
            ValueData::Integer(_) => true,
        }
    }
}