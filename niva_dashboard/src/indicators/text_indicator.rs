use crate::indicators::indicator::{Indicator, IndicatorBounds};
use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::UIStyle;
use crate::hardware::sensor_value::{SensorValue, ValueData};

/// Context-agnostic text indicator that displays sensor values as formatted text.
/// 
/// ## Design Philosophy
/// This indicator is completely detached from context - it doesn't know what it represents
/// or which style values to use. All styling parameters (fonts, colors, sizes) must be 
/// provided externally during construction, making it a pure rendering component.
/// 
/// ## Benefits
/// - **Performance**: No runtime style lookups, all values are pre-resolved
/// - **Flexibility**: Can be styled independently without knowledge of UI context
/// - **Testability**: Easy to test with known style parameters
/// - **Reusability**: Same component can be used with different styling systems
/// 
/// ## Usage
/// ```rust
/// // All styling must be provided upfront
/// let indicator = TextIndicator::new(
///     1,                                    // precision
///     true,                                 // show_unit
///     false,                                // show_label
///     TextAlignment::Center,                // alignment
///     "/path/to/font.ttf".to_string(),     // font_path
///     24,                                   // font_size
///     1.0,                                  // scale
///     (1.0, 1.0, 1.0),                     // primary_color
///     (1.0, 0.65, 0.0),                    // warning_color
///     (1.0, 0.0, 0.0),                     // error_color
/// );
/// ```
pub struct TextIndicator {
    /// Format precision for floating point values
    precision: usize,
    /// Whether to show the unit after the value
    show_unit: bool,
    /// Whether to show the label before the value
    show_label: bool,
    /// Text alignment within bounds
    alignment: TextAlignment,
    /// Font path for text rendering
    font_path: String,
    /// Font size for text rendering
    font_size: u32,
    /// Text scale factor
    scale: f32,
    /// Primary text color (RGB)
    primary_color: (f32, f32, f32),
    /// Warning text color (RGB)
    warning_color: (f32, f32, f32),
    /// Error text color (RGB)
    error_color: (f32, f32, f32),
}

#[derive(Debug, Clone, Copy)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
}

impl TextIndicator {
    /// Create a new text indicator with all styling parameters provided externally
    pub fn new(
        precision: usize,
        show_unit: bool,
        show_label: bool,
        alignment: TextAlignment,
        font_path: String,
        font_size: u32,
        scale: f32,
        primary_color: (f32, f32, f32),
        warning_color: (f32, f32, f32),
        error_color: (f32, f32, f32),
    ) -> Self {
        Self {
            precision,
            show_unit,
            show_label,
            alignment,
            font_path,
            font_size,
            scale,
            primary_color,
            warning_color,
            error_color,
        }
    }

    /// Format the sensor value as a display string (without label)
    fn format_value(&self, value: &SensorValue) -> String {
        let value_str = match value.value {
            ValueData::Empty => "---".to_string(),
            ValueData::Digital(b) => {
                if b { "ВКЛ".to_string() } else { "ВЫКЛ".to_string() }
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
    fn get_text_color(&self, value: &SensorValue) -> (f32, f32, f32) {
        if value.is_critical() {
            self.error_color
        } else if value.is_warning() {
            self.warning_color
        } else {
            self.primary_color
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

impl Indicator for TextIndicator {
    fn with_decorators(self, _decorators: Vec<Box<dyn crate::indicators::decorator::Decorator>>) -> Self {
        // Simple implementation - decorators not yet integrated
        self
    }

    fn render(
        &self,
        value: &SensorValue,
        bounds: IndicatorBounds,
        _style: &UIStyle,
        context: &mut GraphicsContext,
    ) -> Result<(), String> {
        // Get label and value texts
        let label_text = self.get_label(value);
        let value_text = self.format_value(value);
        
        // Use stored style parameters (no lookup needed)
        let text_color = self.get_text_color(value);
        
        // Calculate text dimensions
        let label_width = if !label_text.is_empty() {
            context.calculate_text_width_with_font(
                &label_text,
                self.scale,
                &self.font_path,
                self.font_size,
            )?
        } else {
            0.0
        };
        
        let value_width = context.calculate_text_width_with_font(
            &value_text,
            self.scale,
            &self.font_path,
            self.font_size,
        )?;
        
        // Get font height for positioning
        let font_height = context.get_line_height_with_font(self.scale, &self.font_path, self.font_size)?;
        
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
                self.scale,
                text_color,
                &self.font_path,
                self.font_size,
            )?;
        }
        
        // Render value
        context.render_text_with_font(
            &value_text,
            value_x,
            value_y,
            self.scale,
            text_color,
            &self.font_path,
            self.font_size,
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