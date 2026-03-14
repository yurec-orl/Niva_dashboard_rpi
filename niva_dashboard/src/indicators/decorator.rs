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
    offset_h: f32,
    offset_v: f32,
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
            offset_h: 0.0,
            offset_v: 0.0,
        }
    }

    pub fn with_offset(mut self, offset_h: f32, offset_v: f32) -> Self {
        self.offset_h = offset_h;
        self.offset_v = offset_v;
        self
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
        
        Ok((x + self.offset_h, y + self.offset_v))
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

pub struct ArcDecorator {
    radius: f32,
    thickness: f32,
    color: (f32, f32, f32),
    start_angle: f32,
    end_angle: f32,
}

impl ArcDecorator {
    pub fn new(
        radius: f32,
        thickness: f32,
        color: (f32, f32, f32),
        start_angle: f32,
        end_angle: f32,
    ) -> Self {
        Self {
            radius,
            thickness,
            color,
            start_angle,
            end_angle,
        }
    }
}

impl Decorator for ArcDecorator {
    fn render(
        &self,
        bounds: IndicatorBounds,
        _style: &UIStyle,
        context: &mut GraphicsContext,
    ) -> Result<(), String> {
        // Calculate center point
        let center_x = bounds.x + bounds.width / 2.0;
        let center_y = bounds.y + bounds.height / 2.0;
        
        // Render the arc
        context.render_circle_arc_outline(
            center_x,
            center_y,
            self.radius,
            self.thickness,
            self.color,
            self.start_angle,
            self.end_angle,
            256, // segments
        )?;
        
        Ok(())
    }
}