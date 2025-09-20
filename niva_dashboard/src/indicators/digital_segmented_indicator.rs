use crate::indicators::indicator::{Indicator, IndicatorBounds};
use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;
use crate::hardware::sensor_value::{SensorValue, ValueData};

/// Simple digital numeric indicator using 7-segment fonts
pub struct DigitalSegmentedIndicator {
    /// Number of digits to display
    digits: usize,
    /// Number of decimal places (0 for integers)
    decimals: usize,
    /// Whether to show inactive segments (for realistic 7-segment display look)
    show_inactive_segments: bool,
    /// Color intensity for inactive segments (0.0 = invisible, 1.0 = same as active)
    inactive_intensity: f32,
}

impl DigitalSegmentedIndicator {
    /// Create a new digital indicator
    /// - digits: total number of digits (including decimal places)
    /// - decimals: number of decimal places (0 for integers)
    pub fn new(digits: usize, decimals: usize) -> Self {
        Self { 
            digits, 
            decimals,
            show_inactive_segments: true,
            inactive_intensity: 1.0, // Dim inactive segments to 15% brightness
        }
    }

    /// Create an integer display (e.g., "0123" for 4 digits)
    pub fn integer(digits: usize) -> Self {
        Self::new(digits, 0)
    }

    /// Create a float display (e.g., "12.3" for 3 digits, 1 decimal)
    pub fn float(digits: usize, decimals: usize) -> Self {
        Self::new(digits, decimals)
    }
    
    /// Enable/disable inactive segments display
    pub fn with_inactive_segments(mut self, show: bool) -> Self {
        self.show_inactive_segments = show;
        self
    }
    
    /// Set inactive segment intensity (0.0 = invisible, 1.0 = same as active)
    pub fn with_inactive_intensity(mut self, intensity: f32) -> Self {
        self.inactive_intensity = intensity.clamp(0.0, 1.0);
        self
    }

    /// Format numeric value
    fn format_value(&self, value: f32) -> String {
        if self.decimals == 0 {
            // For integers, don't pad with spaces as DSEG fonts may not handle spaces well
            format!("{}", value as i32)
        } else {
            format!("{:.decimals$}", value, decimals = self.decimals)
        }
    }
    
    /// Generate inactive segments display pattern
    /// For 7-segment displays, show all segments active (8 pattern) to simulate background
    fn generate_inactive_pattern(&self) -> String {
        let digit_pattern = if self.decimals == 0 {
            // For integers, show all 8s
            "8".repeat(self.digits)
        } else {
            // For floats, show 8s with decimal point
            let integer_digits = self.digits - self.decimals - 1; // -1 for decimal point
            format!("{}.{}", "8".repeat(integer_digits), "8".repeat(self.decimals))
        };
        
        digit_pattern
    }
    
    /// Render inactive segments as background
    fn render_inactive_segments(
        &self,
        bounds: IndicatorBounds,
        style: &UIStyle,
        context: &mut GraphicsContext,
        font_path: &str,
        scale: f32,
        font_size: u32,
        inactive_color: (f32, f32, f32),
    ) -> Result<(f32, f32), String> {
        if !self.show_inactive_segments {
            return Ok((0.0, bounds.x));
        }
        
        let inactive_pattern = self.generate_inactive_pattern();
        
        // Calculate text position (centered within bounds)
        let text_width = context.calculate_text_width_with_font(
            &inactive_pattern, scale, font_path, font_size
        )?;
        
        let text_height = context.calculate_text_height_with_font(
            &inactive_pattern, scale, font_path, font_size
        )?;
        
        let x = bounds.x + (bounds.width - text_width) / 2.0;
        let y = bounds.y + (bounds.height - text_height) / 2.0;
        
        // Render inactive segments centered
        context.render_text_with_font(
            &inactive_pattern, x, y, scale, inactive_color, font_path, font_size
        )?;
        
        Ok((text_width, x))
    }
}

impl Default for DigitalSegmentedIndicator {
    fn default() -> Self {
        Self::integer(4).with_inactive_segments(true) // Default to 4-digit integer with inactive segments
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

        // Use DSEG font for 7-segment look
        let font_path = style.get_string(DIGITAL_DISPLAY_FONT, DIGITAL_DISPLAY_FONT_PATH);
        let font_size = style.get_integer(DIGITAL_DISPLAY_FONT_SIZE, 32) as u32;
        let scale = style.get_float(DIGITAL_DISPLAY_SCALE, 2.0);
        
        // Get colors from style
        let active_color = style.get_color(DIGITAL_DISPLAY_ACTIVE_COLOR, (0.0, 0.0, 0.0)); // Black by default

        let mut inactive_color = style.get_color(DIGITAL_DISPLAY_INACTIVE_COLOR, (0.84, 0.41, 0.0));
        inactive_color = (
            inactive_color.0 * self.inactive_intensity,
            inactive_color.1 * self.inactive_intensity,
            inactive_color.2 * self.inactive_intensity,
        );
        
        // Step 1: Render inactive segments as background
        let (inactive_width, inactive_x) = self.render_inactive_segments(bounds, style, context, &font_path, scale, font_size, inactive_color)?;

        // Step 2: Format and render the active value on top
        let formatted_value = self.format_value(numeric_value);

        // Calculate text position (right-aligned within the inactive pattern)
        let text_width = context.calculate_text_width_with_font(
            &formatted_value, scale, &font_path, font_size
        )?;
        
        let text_height = context.calculate_text_height_with_font(
            &formatted_value, scale, &font_path, font_size
        )?;
        
        // Right-align the active text within the centered inactive pattern
        let x = inactive_x + inactive_width - text_width;
        let y = bounds.y + (bounds.height - text_height) / 2.0;

        // Render the active digits
        context.render_text_with_font(
            &formatted_value, x, y, scale, active_color, &font_path, font_size
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
