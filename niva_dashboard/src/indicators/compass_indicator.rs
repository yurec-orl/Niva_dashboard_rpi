#![allow(dead_code)]
use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;
use crate::hardware::sensor_value::{SensorValue, ValueData};
use crate::indicators::decorator::Decorator;
use crate::indicators::indicator::{Indicator, IndicatorBase, IndicatorBounds};
use std::f32::consts::PI;
use std::sync::Once;
use gl;

// Cached shader program and VBO for mark rendering — created once and reused across all
// frames, same rationale as NeedleGaugeMarksDecorator (see needle_indicator.rs):
// glGenBuffers/glDeleteBuffers inside a per-frame render path leaks on the RPi V3D driver.
static mut MARK_SHADER_PROGRAM: u32 = 0;
static MARK_SHADER_INIT: Once = Once::new();
static mut MARKS_VBO: u32 = 0;
static MARKS_VBO_INIT: Once = Once::new();

/// Screen-space angle (radians) pointing straight up, in this codebase's convention where
/// angle 0 = +x (right) and angle increases clockwise (screen y grows downward, so a
/// positive sin() moves a point down the screen).
const UP_ANGLE: f32 = -PI / 2.0;

/// Rotating compass heading scale: a semi-circular tape of heading marks (major every 10°,
/// minor every 5°, labeled every 30°) that rotates as the sensor value (heading, degrees)
/// changes, so the mark matching the current heading always sits under the fixed lubber line
/// drawn by CompassHeadingMarkerDecorator. Only the rotating tape is drawn here — the fixed
/// overlay doesn't depend on the sensor value at all, so it lives in its own Decorator
/// instead of being part of this indicator's render path.
///
/// Rendering re-generates the visible marks' vertices every frame (like every other indicator
/// in this codebase — see NeedleGaugeMarksDecorator, GaugeIndicator). A precomputed "whole
/// circle, rotated via a shader uniform" version was considered, but at most ~48 marks are
/// ever visible at once (240°/5°) — run_fuel_level_grid_test already demonstrates this GL
/// path handling 50 full gauges (marks + needle + numbers each) per frame without measurable
/// per-frame cost growth, so a handful of batched quads here is not worth the extra shader
/// complexity unless profiling ever says otherwise.
pub struct CompassIndicator {
    /// Half-width of the visible arc, in degrees from straight up on each side. 120° (the
    /// default) shows 2/3 of the full circle: the whole front hemisphere plus 30° into the
    /// rear hemisphere on each side — i.e. up to the marks reading heading±120°.
    visible_half_angle_deg: f32,
    major_mark_step_deg: f32,
    minor_mark_step_deg: f32,
    label_step_deg: f32,
    major_mark_length: f32,
    minor_mark_length: f32,
    mark_width: f32,
    /// Gap between the computed pie radius and the outer edge of the marks, so mark tips
    /// don't touch the indicator bounds.
    ring_margin: f32,
    label_font_size: u32,
    /// Gap between the inner edge of the major marks and the outer edge of labels.
    label_gap: f32,
    major_mark_color_key: &'static str,
    minor_mark_color_key: &'static str,
    label_color_key: &'static str,
    base: IndicatorBase,
}

impl CompassIndicator {
    pub fn new() -> Self {
        Self {
            visible_half_angle_deg: 120.0,
            major_mark_step_deg: 10.0,
            minor_mark_step_deg: 5.0,
            label_step_deg: 30.0,
            major_mark_length: 18.0,
            minor_mark_length: 10.0,
            mark_width: 2.0,
            ring_margin: 6.0,
            label_font_size: 32,
            label_gap: 24.0,
            major_mark_color_key: COMPASS_MAJOR_MARK_COLOR,
            minor_mark_color_key: COMPASS_MINOR_MARK_COLOR,
            label_color_key: COMPASS_LABEL_COLOR,
            base: IndicatorBase::new(),
        }
    }

    pub fn with_visible_half_angle(mut self, degrees: f32) -> Self {
        self.visible_half_angle_deg = degrees;
        self
    }

    pub fn visible_half_angle_deg(&self) -> f32 {
        self.visible_half_angle_deg
    }

    pub fn ring_margin(&self) -> f32 {
        self.ring_margin
    }

    pub fn major_mark_length(&self) -> f32 {
        self.major_mark_length
    }

    /// Circle center and radius for the visible pie shape within `bounds`.
    ///
    /// The circle center sits below the bounds' vertical midpoint rather than at it: for a
    /// half-angle beyond 90° the visible arc (from -half_angle to +half_angle around straight
    /// up) extends further above the center than below it, so centering the *circle* would
    /// leave the drawn shape lopsided within `bounds`. Centering the shape's own bounding box
    /// instead gives `center_y = bounds_center_y + radius * (1 + cos(half_angle)) / 2`
    /// (derived from the sector's extreme points), with the radius itself capped by whichever
    /// of width/height is tighter.
    ///
    /// Shared between CompassIndicator and CompassHeadingMarkerDecorator (as a pure function
    /// of `bounds` and `visible_half_angle_deg`, not `self`) so the fixed overlay and the
    /// rotating scale can't independently drift out of alignment.
    pub fn geometry(bounds: IndicatorBounds, visible_half_angle_deg: f32) -> (f32, f32, f32) {
        let half_angle = visible_half_angle_deg.to_radians();
        let cos_a = half_angle.cos();
        let radius = (bounds.width / 2.0).min(bounds.height / (1.0 - cos_a));
        let (bounds_cx, bounds_cy) = bounds.center();
        let center_y = bounds_cy + radius * (1.0 + cos_a) / 2.0;
        (bounds_cx, center_y, radius)
    }

    /// Normalizes `v - heading` (both in degrees) into (-180, 180], i.e. the signed angular
    /// distance from the current heading to compass value `v`, shortest way around.
    fn signed_delta_deg(v: f32, heading: f32) -> f32 {
        let mut delta = (v - heading) % 360.0;
        if delta <= -180.0 {
            delta += 360.0;
        } else if delta > 180.0 {
            delta -= 360.0;
        }
        delta
    }

    unsafe fn get_mark_shader() -> u32 {
        MARK_SHADER_INIT.call_once(|| {
            let vertex_shader_source = b"
attribute vec2 position;
attribute vec3 color;
varying vec3 v_color;
void main() {
    gl_Position = vec4(position, 0.0, 1.0);
    v_color = color;
}
\0";

            let fragment_shader_source = b"
precision mediump float;
varying vec3 v_color;
void main() {
    gl_FragColor = vec4(v_color, 1.0);
}
\0";

            let vertex_shader = gl::CreateShader(gl::VERTEX_SHADER);
            let vertex_src_ptr = vertex_shader_source.as_ptr();
            gl::ShaderSource(vertex_shader, 1, &vertex_src_ptr, std::ptr::null());
            gl::CompileShader(vertex_shader);

            let fragment_shader = gl::CreateShader(gl::FRAGMENT_SHADER);
            let fragment_src_ptr = fragment_shader_source.as_ptr();
            gl::ShaderSource(fragment_shader, 1, &fragment_src_ptr, std::ptr::null());
            gl::CompileShader(fragment_shader);

            let program = gl::CreateProgram();
            gl::AttachShader(program, vertex_shader);
            gl::AttachShader(program, fragment_shader);
            gl::LinkProgram(program);

            gl::DeleteShader(vertex_shader);
            gl::DeleteShader(fragment_shader);

            MARK_SHADER_PROGRAM = program;
        });
        MARK_SHADER_PROGRAM
    }

    unsafe fn get_marks_vbo() -> u32 {
        MARKS_VBO_INIT.call_once(|| {
            gl::GenBuffers(1, &raw mut MARKS_VBO);
        });
        MARKS_VBO
    }

    /// Vertices for a single mark (6 vertices x 5 floats), running from `outer_r - len` to
    /// `outer_r` at the given screen-space `angle`.
    #[allow(clippy::too_many_arguments)]
    fn mark_vertices(
        cx: f32, cy: f32, outer_r: f32, len: f32, width: f32, angle: f32,
        screen_w: f32, screen_h: f32, color: (f32, f32, f32),
    ) -> [f32; 30] {
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let inner_r = outer_r - len;

        let inner_x = cx + cos_a * inner_r;
        let inner_y = cy + sin_a * inner_r;
        let outer_x = cx + cos_a * outer_r;
        let outer_y = cy + sin_a * outer_r;

        let perp_x = -sin_a * width * 0.5;
        let perp_y = cos_a * width * 0.5;

        let to_ndc = |x: f32, y: f32| (x / screen_w * 2.0 - 1.0, 1.0 - y / screen_h * 2.0);
        let (i1x, i1y) = to_ndc(inner_x + perp_x, inner_y + perp_y);
        let (i2x, i2y) = to_ndc(inner_x - perp_x, inner_y - perp_y);
        let (o1x, o1y) = to_ndc(outer_x + perp_x, outer_y + perp_y);
        let (o2x, o2y) = to_ndc(outer_x - perp_x, outer_y - perp_y);

        [
            i1x, i1y, color.0, color.1, color.2,
            i2x, i2y, color.0, color.1, color.2,
            o1x, o1y, color.0, color.1, color.2,
            i2x, i2y, color.0, color.1, color.2,
            o2x, o2y, color.0, color.1, color.2,
            o1x, o1y, color.0, color.1, color.2,
        ]
    }

    unsafe fn render_batched_marks(vertices: &[f32], shader_program: u32) {
        if vertices.is_empty() {
            return;
        }
        let vbo = Self::get_marks_vbo();
        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (vertices.len() * std::mem::size_of::<f32>()) as isize,
            vertices.as_ptr() as *const _,
            gl::DYNAMIC_DRAW,
        );

        let pos_attr = gl::GetAttribLocation(shader_program, b"position\0".as_ptr());
        let color_attr = gl::GetAttribLocation(shader_program, b"color\0".as_ptr());

        gl::EnableVertexAttribArray(pos_attr as u32);
        gl::VertexAttribPointer(pos_attr as u32, 2, gl::FLOAT, gl::FALSE, 20, std::ptr::null());
        gl::EnableVertexAttribArray(color_attr as u32);
        gl::VertexAttribPointer(color_attr as u32, 3, gl::FLOAT, gl::FALSE, 20, (8) as *const _);

        let vertex_count = (vertices.len() / 5) as i32;
        gl::DrawArrays(gl::TRIANGLES, 0, vertex_count);
    }
}

impl Indicator for CompassIndicator {
    fn with_decorators(mut self, decorators: Vec<Box<dyn Decorator>>) -> Self {
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
        let heading = value.as_f32().rem_euclid(360.0);
        let (cx, cy, radius) = Self::geometry(bounds, self.visible_half_angle_deg);
        let outer_r = radius - self.ring_margin;

        let major_color = context.apply_brightness(style.get_color(self.major_mark_color_key, (0.9, 0.9, 1.0)));
        let minor_color = context.apply_brightness(style.get_color(self.minor_mark_color_key, (0.5, 0.5, 0.6)));

        let minor_per_major = (self.major_mark_step_deg / self.minor_mark_step_deg).round().max(1.0) as i32;
        let steps = (360.0 / self.minor_mark_step_deg).round() as i32;

        let mut vertices = Vec::with_capacity(steps as usize * 6 * 5);
        for i in 0..steps {
            let v = i as f32 * self.minor_mark_step_deg;
            let delta = Self::signed_delta_deg(v, heading);
            if delta.abs() > self.visible_half_angle_deg {
                continue;
            }
            let is_major = i % minor_per_major == 0;
            let (len, color) = if is_major {
                (self.major_mark_length, major_color)
            } else {
                (self.minor_mark_length, minor_color)
            };
            let screen_angle = UP_ANGLE + delta.to_radians();
            vertices.extend_from_slice(&Self::mark_vertices(
                cx, cy, outer_r, len, self.mark_width, screen_angle,
                context.width as f32, context.height as f32, color,
            ));
        }

        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            let shader_program = Self::get_mark_shader();
            gl::UseProgram(shader_program);
            Self::render_batched_marks(&vertices, shader_program);
        }

        // Labels every label_step_deg, inboard of the major marks.
        let label_color = style.get_color(self.label_color_key, (0.9, 0.9, 1.0));
        let label_font = style.get_string(COMPASS_LABEL_FONT, DEFAULT_GLOBAL_FONT_PATH);
        let label_radius = outer_r - self.major_mark_length - self.label_gap;
        let label_steps = (360.0 / self.label_step_deg).round() as i32;
        for i in 0..label_steps {
            let v = i as f32 * self.label_step_deg;
            let delta = Self::signed_delta_deg(v, heading);
            if delta.abs() > self.visible_half_angle_deg {
                continue;
            }
            let screen_angle = UP_ANGLE + delta.to_radians();
            let lx = cx + screen_angle.cos() * label_radius;
            let ly = cy + screen_angle.sin() * label_radius;
            let text = format!("{:.0}", v);
            let (text_width, text_height) = context.calculate_text_dimensions_with_font(&text, 1.0, &label_font, self.label_font_size)?;
            context.render_text_with_font(
                &text, lx - text_width / 2.0, ly - text_height / 2.0, 1.0, label_color, &label_font, self.label_font_size,
            )?;
        }

        // Decorators (the fixed heading marker) render last so they sit on top of the tape.
        self.base.render_decorators(bounds, style, context)?;

        Ok(())
    }

    fn indicator_type(&self) -> &'static str {
        "CompassIndicator"
    }

    fn supports_value_type(&self, value: &ValueData) -> bool {
        matches!(value, ValueData::Analog(_))
    }
}

/// Fixed heading marker (lubber line): the part of the compass that never rotates. Two
/// parallel vertical bars, `arrow_gap` pixels apart, running from the circle's center up to
/// the inner edge of the rotating mark ring, plus a taller tick centered between them that
/// pokes up into the ring — twice the length of a major mark, per spec.
///
/// `visible_half_angle_deg`/`ring_margin`/`major_mark_length` must match whatever
/// CompassIndicator instance this decorates: both derive their layout from
/// CompassIndicator::geometry given the same bounds, but as separate objects composed by the
/// caller (same pattern as NeedleIndicator + NeedleGaugeMarksDecorator) they don't share state
/// to enforce that automatically.
pub struct CompassHeadingMarkerDecorator {
    visible_half_angle_deg: f32,
    ring_margin: f32,
    major_mark_length: f32,
    arrow_gap: f32,
    arrow_width: f32,
    arrow_color_key: &'static str,
    center_line_color_key: &'static str,
}

impl CompassHeadingMarkerDecorator {
    pub fn new(visible_half_angle_deg: f32, ring_margin: f32, major_mark_length: f32) -> Self {
        Self {
            visible_half_angle_deg,
            ring_margin,
            major_mark_length,
            arrow_gap: 20.0,
            arrow_width: 3.0,
            arrow_color_key: COMPASS_ARROW_COLOR,
            center_line_color_key: COMPASS_CENTER_LINE_COLOR,
        }
    }

    pub fn with_arrow_gap(mut self, pixels: f32) -> Self {
        self.arrow_gap = pixels;
        self
    }

    pub fn with_arrow_width(mut self, pixels: f32) -> Self {
        self.arrow_width = pixels;
        self
    }
}

impl Decorator for CompassHeadingMarkerDecorator {
    fn render(&self, bounds: IndicatorBounds, style: &UIStyle, context: &mut GraphicsContext) -> Result<(), String> {
        let (cx, cy, radius) = CompassIndicator::geometry(bounds, self.visible_half_angle_deg);
        let outer_r = radius - self.ring_margin;
        let marks_inner_r = outer_r - self.major_mark_length;

        let arrow_color = style.get_color(self.arrow_color_key, (1.0, 0.5, 0.0));
        let center_color = style.get_color(self.center_line_color_key, (1.0, 0.5, 0.0));

        let half_gap = self.arrow_gap / 2.0;

        let top_y = cy - marks_inner_r + self.major_mark_length * 2.0;

        // Two vertical halves, from the circle's center up to the scale's inner edge.
        context.render_rectangle(cx - half_gap - self.arrow_width / 2.0, top_y, self.arrow_width, bounds.height - top_y, arrow_color, true, 0.0, 0.0)?;
        context.render_rectangle(cx + half_gap - self.arrow_width / 2.0, top_y, self.arrow_width, bounds.height - top_y, arrow_color, true, 0.0, 0.0)?;

        // Center tick between the halves, poking up into the scale.
        let center_len = self.major_mark_length * 3.0;
        context.render_rectangle(cx - self.arrow_width / 2.0, top_y - center_len, self.arrow_width, center_len, center_color, true, 0.0, 0.0)?;

        Ok(())
    }
}

/// HDOP quality ring overlaid on the compass, centered on the same point as the compass's own
/// geometry. Bigger ring = worse fix. Unlike the other compass decorations, this isn't a
/// Decorator: HDOP changes every frame with the GNSS fix, but Decorator::render only gets
/// `bounds`/`style`, not a sensor value — so this is a plain helper the page calls directly
/// with a freshly read HDOP, the same way GnssPage re-reads heading each frame.
///
/// Radii are fixed pixel values, not derived from compass geometry, per spec: excellent fits
/// inside the lubber line's two vertical bars (default `arrow_gap` 20 / `arrow_width` 3, so the
/// gap between their inner edges is 17px — 8px radius clears it); good is sized to overlap the
/// outer edge of each bar by 4px (bars' outer edges sit at +/-11.5px from center, so radius
/// 15.5); moderate is ~half the compass's pie radius at the PNP page's bounds (radius 240 for
/// an 800x480 display per GnssPage::render_pnp_mode, so 120); poor sits just inside the mark
/// ring (outer_r 234 at that same radius/ring_margin, so 224).
pub struct HdopIndicator {
    excellent_max: f32,
    good_max: f32,
    moderate_max: f32,
    excellent_radius: f32,
    good_radius: f32,
    moderate_radius: f32,
    poor_radius: f32,
    thickness: f32,
    excellent_color_key: &'static str,
    good_color_key: &'static str,
    moderate_color_key: &'static str,
    poor_color_key: &'static str,
}

impl HdopIndicator {
    pub fn new() -> Self {
        Self {
            excellent_max: 2.0,
            good_max: 5.0,
            moderate_max: 20.0,
            excellent_radius: 8.0,
            good_radius: 15.5,
            moderate_radius: 120.0,
            poor_radius: 224.0,
            thickness: 3.0,
            excellent_color_key: COMPASS_HDOP_EXCELLENT_COLOR,
            good_color_key: COMPASS_HDOP_GOOD_COLOR,
            moderate_color_key: COMPASS_HDOP_MODERATE_COLOR,
            poor_color_key: COMPASS_HDOP_POOR_COLOR,
        }
    }

    /// Draws the ring for `hdop` centered at `(cx, cy)`; draws nothing if no fix is available.
    pub fn render(
        &self, cx: f32, cy: f32, hdop: Option<f32>, style: &UIStyle, context: &mut GraphicsContext,
    ) -> Result<(), String> {
        let Some(hdop) = hdop else { return Ok(()); };

        let (radius, color_key) = if hdop <= self.excellent_max {
            (self.excellent_radius, self.excellent_color_key)
        } else if hdop <= self.good_max {
            (self.good_radius, self.good_color_key)
        } else if hdop <= self.moderate_max {
            (self.moderate_radius, self.moderate_color_key)
        } else {
            (self.poor_radius, self.poor_color_key)
        };

        let color = style.get_color(color_key, (1.0, 1.0, 1.0));
        context.render_circle_arc_outline(cx, cy, radius, self.thickness, color, 0.0, 2.0 * PI, 96)?;

        Ok(())
    }
}
