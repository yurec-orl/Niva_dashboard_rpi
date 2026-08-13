#![allow(dead_code)]
use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;
use crate::hardware::sensor_value::{SensorValue, ValueData};
use crate::indicators::decorator::Decorator;
use crate::indicators::indicator::{Indicator, IndicatorBase, IndicatorBounds};

/// Aircraft-style artificial horizon / pitch ladder. The sensor value is the current pitch
/// angle in degrees (positive = nose up). Rendered so the mark matching the current pitch
/// always sits at the vertical center of `bounds` -- as pitch increases, the whole ladder
/// (sky/ground split, marks, labels) shifts down, the same way a fixed aircraft symbol would
/// appear to rise relative to a horizon painted on a scrolling tape.
pub struct PitchIndicator {
    /// Total degrees of pitch spanned by the full height of `bounds`, so the ladder's mark
    /// spacing scales with whatever size the caller gives it rather than assuming a fixed
    /// pixel scale.
    visible_span_deg: f32,
    mark_step_deg: f32,
    mark_width: f32,
    /// Gap kept between the ladder marks and the left/right edges of `bounds`.
    edge_margin: f32,
    /// Gap kept between each mark line and the label centered between them.
    label_gap: f32,
    base: IndicatorBase,
}

impl PitchIndicator {
    pub fn new() -> Self {
        Self {
            visible_span_deg: 60.0,
            mark_step_deg: 10.0,
            mark_width: 4.0,
            edge_margin: 20.0,
            label_gap: 30.0,
            base: IndicatorBase::new(),
        }
    }

    pub fn with_visible_span_deg(mut self, degrees: f32) -> Self {
        self.visible_span_deg = degrees;
        self
    }
}

impl Indicator for PitchIndicator {
    fn with_decorators(mut self, decorators: Vec<Box<dyn Decorator>>) -> Self {
        self.base.decorators = decorators;
        self
    }

    fn render(&self, value: &SensorValue, bounds: IndicatorBounds, style: &UIStyle, context: &mut GraphicsContext) -> Result<(), String> {
        let pitch_deg = value.as_f32();
        let pitch_deg = if pitch_deg.is_finite() { pitch_deg } else { 0.0 };

        let sky_color = style.get_color(PITCH_SKY_COLOR, (0.42, 0.64, 0.85));
        let ground_color = style.get_color(PITCH_GROUND_COLOR, (0.45, 0.29, 0.16));
        let above_label_color = style.get_color(PITCH_ABOVE_HORIZON_LABEL_COLOR, (0.0, 0.0, 0.0));
        let below_label_color = style.get_color(PITCH_BELOW_HORIZON_LABEL_COLOR, (1.0, 1.0, 1.0));
        let border_color = style.get_color(PITCH_BORDER_COLOR, (1.0, 1.0, 1.0));
        let border_width = style.get_float(PITCH_BORDER_WIDTH, 2.0);
        let font = style.get_string(PITCH_LABEL_FONT, DEFAULT_GLOBAL_FONT_PATH);
        let font_size = style.get_integer(PITCH_LABEL_FONT_SIZE, 42);

        let (cx, cy) = bounds.center();
        let top = bounds.y;
        let bottom = bounds.y + bounds.height;
        let pixels_per_deg = bounds.height / self.visible_span_deg;

        // Horizon (0-degree mark) screen position: sits at the vertical center when
        // pitch_deg is 0, and moves down as pitch_deg (nose up) increases.
        let horizon_y = (cy + pitch_deg * pixels_per_deg).clamp(top, bottom);

        if horizon_y > top {
            context.fill_rect(bounds.x, top, bounds.width, horizon_y - top, sky_color)?;
        }
        if horizon_y < bottom {
            context.fill_rect(bounds.x, horizon_y, bounds.width, bottom - horizon_y, ground_color)?;
        }

        let half_span = self.visible_span_deg / 2.0 + self.mark_step_deg;
        let min_mark = ((pitch_deg - half_span) / self.mark_step_deg).floor() * self.mark_step_deg;
        let max_mark = ((pitch_deg + half_span) / self.mark_step_deg).ceil() * self.mark_step_deg;

        let mut mark = min_mark;
        while mark <= max_mark {
            if mark != 0.0 {
                let y = cy + (pitch_deg - mark) * pixels_per_deg;
                if y >= top && y <= bottom {
                    let color = if mark > 0.0 { above_label_color } else { below_label_color };
                    let label = format!("{}", mark.abs() as i32);

                    let label_width = context.calculate_text_width_with_font(&label, 1.0, &font, font_size)?;
                    let label_line_height = context.get_line_height_with_font(1.0, &font, font_size)?;
                    let label_x = cx - label_width / 2.0;
                    let label_y = y - label_line_height / 2.0;

                    let left_x2 = label_x - self.label_gap;
                    let left_x1 = bounds.x + self.edge_margin;
                    if left_x2 > left_x1 {
                        context.fill_rect(left_x1, y - self.mark_width / 2.0, left_x2 - left_x1, self.mark_width, color)?;
                    }

                    let right_x1 = label_x + label_width + self.label_gap;
                    let right_x2 = bounds.x + bounds.width - self.edge_margin;
                    if right_x2 > right_x1 {
                        context.fill_rect(right_x1, y - self.mark_width / 2.0, right_x2 - right_x1, self.mark_width, color)?;
                    }

                    context.render_text_with_font(&label, label_x, label_y, 1.0, color, &font, font_size)?;
                }
            }
            mark += self.mark_step_deg;
        }

        context.stroke_rect(bounds.x, bounds.y, bounds.width, bounds.height, border_color, border_width)?;

        self.base.render_decorators(bounds, style, context)?;

        Ok(())
    }

    fn indicator_type(&self) -> &'static str {
        "pitch"
    }

    fn supports_value_type(&self, value: &ValueData) -> bool {
        matches!(value, ValueData::Analog(_))
    }
}
