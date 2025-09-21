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

pub struct VerticalBarGuideDecorator {
    labels: Vec<String>,
    font_path: String,
    font_size: u32,
    color: (f32, f32, f32),
    alignment_h: DecoratorAlignmentH,
}

impl VerticalBarGuideDecorator {
    pub fn new(
        labels: Vec<String>,
        font_path: String,
        font_size: u32,
        color: (f32, f32, f32),
        alignment_h: DecoratorAlignmentH,
    ) -> Self {
        Self {
            labels,
            font_path,
            font_size,
            color,
            alignment_h,
        }
    }
}

impl Decorator for VerticalBarGuideDecorator {
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
        
        let segment_height = bounds.height / segment_count as f32;
        
        for (i, label) in self.labels.iter().enumerate() {
            let y = bounds.y + i as f32 * segment_height + (segment_height - self.font_size as f32) / 2.0;
            let x = match self.alignment_h {
                DecoratorAlignmentH::Left => bounds.x - 5.0 - context.calculate_text_width_with_font(label, 1.0, &self.font_path, self.font_size)?,
                DecoratorAlignmentH::Right => bounds.x + bounds.width + 5.0,
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