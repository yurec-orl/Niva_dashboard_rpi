#![allow(dead_code)]
use crate::indicators::indicator::{Indicator, IndicatorBase, IndicatorBounds};
use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::{UIStyle, DEFAULT_GLOBAL_FONT_PATH, DEFAULT_GLOBAL_FONT_SIZE};
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
/// // Defaults cover the common case; override only what differs.
/// let indicator = TextIndicator::new()
///     .with_precision(1)
///     .with_font("/path/to/font.ttf".to_string(), 24)
///     .with_colors((1.0, 1.0, 1.0), (1.0, 0.65, 0.0), (1.0, 0.0, 0.0));
/// ```
pub struct TextIndicator {
    /// Format precision for floating point values
    precision: usize,
    /// Whether to show the unit after the value
    show_unit: bool,
    /// Whether to show the label before the value
    show_label: bool,
    /// Whether to show the formatted value. Off for indicators that only ever display a
    /// static label (e.g. a link-status box showing just "ГНСС" in a status color) — the
    /// underlying SensorValue still drives get_text_color, just isn't rendered as text.
    show_value: bool,
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
    base: IndicatorBase,
}

#[derive(Debug, Clone, Copy)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
}

impl TextIndicator {
    /// Create a text indicator with sensible defaults: precision 0, unit and label shown,
    /// centered, global default font at scale 1.0, white/yellow/red status colors.
    /// Use the `with_...` methods to override any of these.
    pub fn new() -> Self {
        Self {
            precision: 0,
            show_unit: true,
            show_label: true,
            show_value: true,
            alignment: TextAlignment::Center,
            font_path: DEFAULT_GLOBAL_FONT_PATH.to_string(),
            font_size: DEFAULT_GLOBAL_FONT_SIZE,
            scale: 1.0,
            primary_color: (1.0, 1.0, 1.0),
            warning_color: (1.0, 1.0, 0.0),
            error_color: (1.0, 0.0, 0.0),
            base: IndicatorBase::new(),
        }
    }

    pub fn with_precision(mut self, precision: usize) -> Self {
        self.precision = precision;
        self
    }

    pub fn with_parameters(mut self, alignment: TextAlignment, show_unit: bool, show_label: bool, show_value: bool) -> Self {
        self.alignment = alignment;
        self.show_unit = show_unit;
        self.show_label = show_label;
        self.show_value = show_value;
        self
    }

    pub fn with_font(mut self, font_path: String, font_size: u32, scale: f32) -> Self {
        self.font_path = font_path;
        self.font_size = font_size;
        self.scale = scale;
        self
    }

    pub fn with_colors(
        mut self,
        primary_color: (f32, f32, f32),
        warning_color: (f32, f32, f32),
        error_color: (f32, f32, f32),
    ) -> Self {
        self.primary_color = primary_color;
        self.warning_color = warning_color;
        self.error_color = error_color;
        self
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
    
    /// Calculate text position for label and value. When both are shown, label sits above
    /// value, both centered; when only one is shown, it's centered alone in `bounds`.
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

        let center_y = bounds.y + bounds.height / 2.0;

        if self.show_label && self.show_value {
            let spacing = font_height * 0.2; // Small spacing between label and value
            let label_y = center_y - spacing / 2.0 - font_height / 2.0;
            let value_y = center_y + spacing / 2.0 + font_height / 2.0;
            ((label_x, label_y), (value_x, value_y))
        } else {
            let y = center_y - font_height / 2.0;
            ((label_x, y), (value_x, y))
        }
    }
}

impl Indicator for TextIndicator {
    fn with_decorators(mut self, decorators: Vec<Box<dyn crate::indicators::decorator::Decorator>>) -> Self {
        self.base.decorators = decorators;
        self
    }

    fn render(
        &self,
        value: &SensorValue,
        bounds: IndicatorBounds,
        style: &UIStyle,
        context: &mut GraphicsContext,
    ) -> Result<(), String> {
        // Render decorators first, then the text itself on top
        self.base.render_decorators(bounds, style, context)?;

        // Get label and value texts
        let label_text = if self.show_label { self.get_label(value) } else { "".to_string() };
        let value_text = if self.show_value { self.format_value(value) } else { "".to_string() };
        
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
        if !value_text.is_empty() {
            context.render_text_with_font(
                &value_text,
                value_x,
                value_y,
                self.scale,
                text_color,
                &self.font_path,
                self.font_size,
            )?;
        }
        
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