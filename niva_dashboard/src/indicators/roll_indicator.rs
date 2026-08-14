#![allow(dead_code)]
use std::f32::consts::PI;

use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;
use crate::hardware::sensor_value::{SensorValue, ValueData};
use crate::indicators::decorator::Decorator;
use crate::indicators::indicator::{Indicator, IndicatorBase, IndicatorBounds};

const SEGMENT_ANGLE_DEG: f32 = 30.0;

/// Fixed-shape roll (bank) pointer that rotates around its own pivot point -- a 6-segment
/// zigzag (flat / down / up / down / up / flat, each segment of equal horizontal width) whose
/// center vertex (where the first "up" segment meets the second "down" segment) is the pivot.
/// Since that center vertex sits at the same height as the two flat end segments, at roll = 0
/// the whole shape reads as a level line with two symmetric notches -- the classic ADI bank
/// pointer silhouette.
///
/// `bounds.center()` is the pivot -- callers place this indicator so that point lands exactly
/// on PitchIndicator's center, so the pivot doubles as the pitch ladder's fixed reference mark.
/// `bounds.width` is the shape's total width (6x one segment's width); `bounds.height` is
/// unused (the shape has no meaningful height of its own beyond what the geometry produces).
pub struct RollIndicator {
    line_width: f32,
    /// Segments 3 and 4 (the two meeting at the pivot) are trimmed to stop this many pixels
    /// from the pivot, and a circle of this radius is drawn there instead -- so the lines
    /// don't cross inside it.
    pivot_circle_radius: f32,
    base: IndicatorBase,
}

impl RollIndicator {
    pub fn new() -> Self {
        Self {
            line_width: 4.0,
            pivot_circle_radius: 10.0,
            base: IndicatorBase::new(),
        }
    }
}

impl Indicator for RollIndicator {
    fn with_decorators(mut self, decorators: Vec<Box<dyn Decorator>>) -> Self {
        self.base.decorators = decorators;
        self
    }

    fn render(&self, value: &SensorValue, bounds: IndicatorBounds, style: &UIStyle, context: &mut GraphicsContext) -> Result<(), String> {
        let roll_deg = value.as_f32();
        let roll_deg = if roll_deg.is_finite() { roll_deg } else { 0.0 };

        let color = style.get_color(ROLL_INDICATOR_COLOR, (1.0, 1.0, 0.0));

        let (cx, cy) = bounds.center();
        let w = bounds.width / 6.0;
        let seg_angle = SEGMENT_ANGLE_DEG.to_radians();
        let dy = w * seg_angle.tan();

        // Direction unit vectors along the two segments meeting at the pivot (local, pre-
        // rotation): "in" runs from the up-segment's outer end toward the pivot, "out" runs
        // from the pivot toward the down-segment's outer end.
        let dir_in = (seg_angle.cos(), -seg_angle.sin());
        let dir_out = (seg_angle.cos(), seg_angle.sin());
        let r = self.pivot_circle_radius;

        // Local (pre-rotation) vertices, pivot at the origin.
        let p0 = (-3.0 * w, 0.0);
        let p1 = (-2.0 * w, 0.0);
        let p2 = (-w, dy);
        let p3_in = (-dir_in.0 * r, -dir_in.1 * r);   // trimmed end of the up segment
        let p3_out = (dir_out.0 * r, dir_out.1 * r);  // trimmed start of the down segment
        let p4 = (w, dy);
        let p5 = (2.0 * w, 0.0);
        let p6 = (3.0 * w, 0.0);

        // Positive roll rotates the shape clockwise on screen -- BNO085 roll_deg's physical
        // sign depends on board mounting, same caveat as Bno085Orientation's own doc comment.
        let theta = roll_deg.to_radians();
        let (sin_t, cos_t) = theta.sin_cos();
        let rotate = |(x, y): (f32, f32)| -> (f32, f32) {
            (cx + x * cos_t - y * sin_t, cy + x * sin_t + y * cos_t)
        };

        let points: [(f32, f32); 8] = [p0, p1, p2, p3_in, p3_out, p4, p5, p6]
            .map(rotate);
        let [p0, p1, p2, p3_in, p3_out, p4, p5, p6] = points;

        context.render_line(p0, p1, color, self.line_width)?;
        context.render_line(p1, p2, color, self.line_width)?;
        context.render_line(p2, p3_in, color, self.line_width)?;
        context.render_line(p3_out, p4, color, self.line_width)?;
        context.render_line(p4, p5, color, self.line_width)?;
        context.render_line(p5, p6, color, self.line_width)?;

        context.render_circle_arc_outline(cx, cy, r, self.line_width, color, 0.0, 2.0 * PI, 32)?;

        self.base.render_decorators(bounds, style, context)?;

        Ok(())
    }

    fn indicator_type(&self) -> &'static str {
        "roll"
    }

    fn supports_value_type(&self, value: &ValueData) -> bool {
        matches!(value, ValueData::Analog(_))
    }
}

/// Fixed bank-angle scale flanking the roll pointer: two symmetric arcs, one on each side of
/// the pivot, starting level with the pivot (0 deg) and curving downward to `max_angle_deg`.
/// Doesn't rotate with roll -- like CompassHeadingMarkerDecorator, it's the fixed reference the
/// rotating pointer above reads against.
///
/// Radius is fixed at half of `bounds.width` per spec (RollIndicator's own pivot/width, so the
/// scale wraps snugly around the pointer shape) rather than being independently configurable.
pub struct RollScaleDecorator {
    max_angle_deg: f32,
    minor_step_deg: f32,
    major_step_deg: f32,
    minor_mark_length: f32,
    major_mark_length: f32,
    mark_width: f32,
    /// Gap kept between the outer end of a major mark and its label -- labels sit further
    /// from the pivot than the marks ("outside" the scale), the mirror of e.g. CompassIndicator
    /// which places labels inboard of its marks.
    label_gap: f32,
}

impl RollScaleDecorator {
    pub fn new() -> Self {
        Self {
            max_angle_deg: 45.0,
            minor_step_deg: 5.0,
            major_step_deg: 15.0,
            minor_mark_length: 8.0,
            major_mark_length: 14.0,
            mark_width: 2.0,
            label_gap: 20.0,
        }
    }
}

impl Decorator for RollScaleDecorator {
    fn render(&self, bounds: IndicatorBounds, style: &UIStyle, context: &mut GraphicsContext) -> Result<(), String> {
        let (cx, cy) = bounds.center();
        let radius = bounds.width / 2.0;

        let mark_color = style.get_color(ROLL_SCALE_COLOR, (1.0, 1.0, 1.0));
        let label_font = style.get_string(ROLL_SCALE_LABEL_FONT, DEFAULT_GLOBAL_FONT_PATH);
        let label_font_size = style.get_integer(ROLL_SCALE_LABEL_FONT_SIZE, 32);

        let steps = (self.max_angle_deg / self.minor_step_deg).round() as i32;

        // side = 1.0 sweeps 0 -> max_angle_deg on the right (screen angle 0 = +x, increasing
        // clockwise, so this curves downward); side = -1.0 mirrors it on the left (screen
        // angle PI = -x, decreasing toward PI - max, also curving downward).
        for side in [1.0f32, -1.0f32] {
            for i in 0..=steps {
                let offset_deg = i as f32 * self.minor_step_deg;
                let is_major = (offset_deg % self.major_step_deg).abs() < 0.01;

                let angle = if side > 0.0 { offset_deg.to_radians() } else { PI - offset_deg.to_radians() };
                let (cos_a, sin_a) = (angle.cos(), angle.sin());

                let mark_length = if is_major { self.major_mark_length } else { self.minor_mark_length };
                let p1 = (cx + cos_a * radius, cy + sin_a * radius);
                let p2 = (cx + cos_a * (radius + mark_length), cy + sin_a * (radius + mark_length));
                context.render_line(p1, p2, mark_color, self.mark_width)?;

                if is_major {
                    let label = format!("{}", offset_deg as i32);
                    let label_radius = radius + mark_length + self.label_gap;
                    let lx = cx + cos_a * label_radius;
                    let ly = cy + sin_a * label_radius;
                    let label_width = context.calculate_text_width_with_font(&label, 1.0, &label_font, label_font_size)?;
                    let label_height = context.get_line_height_with_font(1.0, &label_font, label_font_size)?;
                    context.render_text_with_font(&label, lx - label_width / 2.0, ly - label_height / 2.0, 1.0, mark_color, &label_font, label_font_size)?;
                }
            }
        }

        Ok(())
    }
}
