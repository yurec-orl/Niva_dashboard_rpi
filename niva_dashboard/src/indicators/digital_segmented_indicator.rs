use crate::indicators::indicator::{Indicator, IndicatorBounds};
use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::{UIStyle, DIGITAL_DISPLAY_FONT_PATH};
use crate::hardware::sensor_value::{SensorValue, ValueData};

/// Simple digital numeric indicator using 7-segment fonts
pub struct DigitalSegmentedIndicator {
    /// Number of digits to display
    digits: usize,
    /// Number of decimal places (0 for integers)
    decimals: usize,
}

impl DigitalSegmentedIndicator {
    /// Create a new digital indicator
    /// - digits: total number of digits (including decimal places)
    /// - decimals: number of decimal places (0 for integers)
    pub fn new(digits: usize, decimals: usize) -> Self {
        Self { digits, decimals }
    }

    /// Create an integer display (e.g., "0123" for 4 digits)
    pub fn integer(digits: usize) -> Self {
        Self::new(digits, 0)
    }

    /// Create a float display (e.g., "12.3" for 3 digits, 1 decimal)
    pub fn float(digits: usize, decimals: usize) -> Self {
        Self::new(digits, decimals)
    }

    /// Format numeric value
    fn format_value(&self, value: f32) -> String {
        if self.decimals == 0 {
            format!("{:0width$}", value as i32, width = self.digits)
        } else {
            format!("{:0width$.decimals$}", value, width = self.digits, decimals = self.decimals)
        }
    }
}

impl Default for DigitalSegmentedIndicator {
    fn default() -> Self {
        Self::integer(4) // Default to 4-digit integer display
    }
}

impl Indicator for DigitalSegmentedIndicator {
    fn render(
        &self,
        value: &SensorValue,
        bounds: IndicatorBounds,
        style: &UIStyle,
        context: &mut GraphicsContext,
    ) -> Result<(), String> {
        // Extract numeric value
        let numeric_value = match &value.value {
            ValueData::Analog(v) => *v,
            ValueData::Integer(i) => *i as f32,
            ValueData::Percentage(p) => *p,
            _ => return Ok(()), // Skip non-numeric values
        };

        // Format the value
        let formatted_value = self.format_value(numeric_value);

        // Use DSEG font for 7-segment look
        let font_path = style.get_string("digital_font_path", DIGITAL_DISPLAY_FONT_PATH);
        let font_size = 32u32;
        let color = (1.0, 0.647, 0.0); // Amber color

        // Calculate text position (centered)
        let text_width = context.calculate_text_width_with_font(
            &formatted_value, 1.0, &font_path, font_size
        )?;
        
        let x = bounds.x + (bounds.width - text_width) / 2.0;
        let y = bounds.y + bounds.height / 2.0 + (font_size as f32) / 4.0;

        // Render the text
        context.render_text_with_font(
            &formatted_value, x, y, 1.0, color, &font_path, font_size
        )?;

        Ok(())
    }

    fn indicator_type(&self) -> &'static str {
        "DigitalSegmentedIndicator"
    }

    fn supports_value_type(&self, value: &ValueData) -> bool {
        matches!(value, ValueData::Analog(_) | ValueData::Integer(_) | ValueData::Percentage(_))
    }
}
