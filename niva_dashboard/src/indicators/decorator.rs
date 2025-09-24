use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::UIStyle;
use crate::indicators::IndicatorBounds;

#[derive(Debug, Clone, Copy)]
pub enum DecoratorAlignmentV {
    Top,
    Bottom,
    Center,
}

#[derive(Debug, Clone, Copy)]
pub enum DecoratorAlignmentH {
    Left,
    Right,
    Center,
}

pub trait Decorator {
    /// Render additional decorations around the indicator
    fn render(
        &self,
        bounds: IndicatorBounds,
        style: &UIStyle,
        context: &mut GraphicsContext,
    ) -> Result<(), String>;
}

/// Simple text label decorator
/// Displays a text label at specified position relative to the indicator bounds
pub struct LabelDecorator {
    text: String,
    font_path: String,
    font_size: u32,
    color: (f32, f32, f32),
    alignment_h: DecoratorAlignmentH,
    alignment_v: DecoratorAlignmentV,
}

impl LabelDecorator {
    /// Create a new label decorator
    pub fn new(
        text: String,
        font_path: String,
        font_size: u32,
        color: (f32, f32, f32),
        alignment_h: DecoratorAlignmentH,
        alignment_v: DecoratorAlignmentV,
    ) -> Self {
        Self {
            text,
            font_path,
            font_size,
            color,
            alignment_h,
            alignment_v,
        }
    }

    /// Calculate label position based on bounds and alignment
    fn calculate_position(&self, bounds: &IndicatorBounds, context: &mut GraphicsContext) -> Result<(f32, f32), String> {
        // Get text dimensions
        let text_width = context.calculate_text_width_with_font(&self.text, 1.0, &self.font_path, self.font_size)?;
        let text_height = context.calculate_text_height_with_font(&self.text, 1.0, &self.font_path, self.font_size)?;
        
        // Calculate vertical position
        let y = match self.alignment_v {
            DecoratorAlignmentV::Top => bounds.y - text_height - 5.0, // 5px margin
            DecoratorAlignmentV::Bottom => bounds.y + bounds.height + 5.0,
            DecoratorAlignmentV::Center => bounds.y + (bounds.height - text_height) / 2.0,
        };
        
        // Calculate horizontal position
        let x = match self.alignment_h {
            DecoratorAlignmentH::Left => bounds.x,
            DecoratorAlignmentH::Right => bounds.x + bounds.width - text_width,
            DecoratorAlignmentH::Center => bounds.x + (bounds.width - text_width) / 2.0,
        };
        
        Ok((x, y))
    }
}

impl Decorator for LabelDecorator {
    fn render(
        &self,
        bounds: IndicatorBounds,
        _style: &UIStyle,
        context: &mut GraphicsContext,
    ) -> Result<(), String> {
        // Calculate label position
        let (x, y) = self.calculate_position(&bounds, context)?;
        
        // Render the label
        context.render_text_with_font(
            &self.text,
            x,
            y,
            1.0, // scale
            self.color,
            &self.font_path,
            self.font_size,
        )?;
        
        Ok(())
    }
}

/// Decorator for rendering scale marks and labels vertically alongside a vertical bar indicator
/// Labels are ordered from top to bottom
/// Scale marks are optional and can be enabled during construction
pub struct VerticalBarScaleDecorator {
    labels: Vec<String>,    // Labels for each scale mark - no labels if empty
    font_path: String,
    font_size: u32,
    color: (f32, f32, f32),
    scale_marks: bool,      // Whether to draw scale marks
    marks_color: (f32, f32, f32),
    marks_width: f32,
    marks_thickness: f32,
    alignment_h: DecoratorAlignmentH,
}

impl VerticalBarScaleDecorator {
    /// Create a new vertical bar scale decorator
    /// Labels are ordered from top to bottom
    pub fn new(
        labels: Vec<String>,
        font_path: String,
        font_size: u32,
        color: (f32, f32, f32),
        alignment_h: DecoratorAlignmentH,
    ) -> Self {
        Self {
            labels,
            scale_marks: false,
            font_path,
            font_size,
            color,
            marks_color: (1.0, 1.0, 1.0),
            marks_thickness: 1.0,
            marks_width: 5.0,
            alignment_h,
        }
    }

    /// Enable scale marks with specified color, width and thickness
    pub fn with_scale_marks(mut self, color: (f32, f32, f32), width: f32, thickness: f32) -> Self {
        self.scale_marks = true;
        self.marks_color = color;
        self.marks_width = width;
        self.marks_thickness = thickness;
        self
    }
}

impl Decorator for VerticalBarScaleDecorator {
    fn render(
        &self,
        bounds: IndicatorBounds,
        _style: &UIStyle,
        context: &mut GraphicsContext,
    ) -> Result<(), String> {
        let segment_count = self.labels.len();
        if segment_count == 0 {
            return Ok(()); // Nothing to render
        }
        
        let mut base_x_pos = match self.alignment_h {
            DecoratorAlignmentH::Left => bounds.x - self.marks_thickness, // 5px margin
            DecoratorAlignmentH::Right => bounds.x + bounds.width + self.marks_thickness,
            DecoratorAlignmentH::Center => Err("Center alignment not supported".to_string())?,
        };
        let segment_height = bounds.height / segment_count as f32;

        // Draw scale marks if enabled
        if self.scale_marks {
            base_x_pos += match self.alignment_h {
                DecoratorAlignmentH::Left => -(self.marks_width + self.marks_thickness),
                DecoratorAlignmentH::Right => (self.marks_width + self.marks_thickness),
                DecoratorAlignmentH::Center => 0.0, // Not applicable
            };

            for i in 0..segment_count {
                let y = bounds.y + i as f32 * segment_height + segment_height / 2.0;
                print!("--------- ({}, {})\r\n", i, y);
                context.render_rectangle(base_x_pos, y - self.marks_thickness / 2.0,
                                         self.marks_width, self.marks_thickness,
                                         self.marks_color, true, 1.0, 0.0)?;
            }
            print!("---------{} \r\n", (segment_count - 1) as f32 * segment_height + segment_height / 2.0);
            context.render_rectangle(base_x_pos, bounds.y + segment_height / 2.0,
                                     self.marks_thickness,
                                     (segment_count - 1) as f32 * segment_height,
                                     self.marks_color, true, 1.0, 0.0)?; // Vertical line
        }

        for (i, label) in self.labels.iter().enumerate() {
            let y = bounds.y + i as f32 * segment_height + (segment_height - self.font_size as f32) / 2.0;
            let x = match self.alignment_h {
                DecoratorAlignmentH::Left => base_x_pos - 5.0 - context.calculate_text_width_with_font(label, 1.0, &self.font_path, self.font_size)?,
                DecoratorAlignmentH::Right => base_x_pos + bounds.width + 5.0,
                DecoratorAlignmentH::Center => Err("Center alignment not supported".to_string())?,
            };
            
            context.render_text_with_font(
                label,
                x,
                y,
                1.0, // scale
                self.color,
                &self.font_path,
                self.font_size,
            )?;
        }
        
        Ok(())
    }
}