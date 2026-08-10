/// Produces the vertex geometry for a NeedleIndicator's rotating needle.
pub trait NeedleShape {
    /// Interleaved vertex buffer — [x, y, r, g, b] per vertex, position already
    /// converted to NDC, grouped into whole triangles for GL_TRIANGLES. Length
    /// must be a multiple of 15; callers must size buffers/draw calls from the
    /// returned Vec's length, not assume a fixed count (shapes may draw more
    /// than one disjoint piece, e.g. two small marks instead of one blade).
    ///
    /// Called every frame with the needle's current `angle` (already resolved
    /// from the live sensor value) — there is no separate rotation step applied
    /// afterward. Implementations must rotate their own geometry by `angle`
    /// internally (via `angle.cos()`/`angle.sin()`, as `ArrowNeedleShape` does
    /// below) so the shape tracks the sensor value frame to frame; a shape that
    /// ignores `angle` renders fixed in place instead of rotating.
    #[allow(clippy::too_many_arguments)]
    fn vertices(
        &self,
        center_x: f32,
        center_y: f32,
        length: f32,
        angle: f32,
        color: (f32, f32, f32),
        screen_w: f32,
        screen_h: f32,
    ) -> Vec<f32>;
}

/// Default needle shape: a tapered blade from `base_width` near the center
/// to `tip_width` near the tip.
pub struct ArrowNeedleShape {
    base_width: f32,
    tip_width: f32,
}

impl ArrowNeedleShape {
    pub fn new(base_width: f32, tip_width: f32) -> Self {
        Self { base_width, tip_width }
    }
}

impl NeedleShape for ArrowNeedleShape {
    fn vertices(&self, center_x: f32, center_y: f32, length: f32, angle: f32,
                color: (f32, f32, f32), screen_w: f32, screen_h: f32) -> Vec<f32> {
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        let tip_x = center_x + cos_a * length;
        let tip_y = center_y + sin_a * length;

        let base_width = self.base_width;
        let tip_width = self.tip_width;

        // Base vertices (perpendicular to needle direction)
        let base_perp_cos = (-sin_a) * base_width * 0.5;
        let base_perp_sin = cos_a * base_width * 0.5;

        let base1_x = center_x + base_perp_cos;
        let base1_y = center_y + base_perp_sin;
        let base2_x = center_x - base_perp_cos;
        let base2_y = center_y - base_perp_sin;

        // Tip vertices (perpendicular to needle direction at tip)
        let tip_perp_cos = (-sin_a) * tip_width * 0.5;
        let tip_perp_sin = cos_a * tip_width * 0.5;

        let tip1_x = tip_x + tip_perp_cos;
        let tip1_y = tip_y + tip_perp_sin;
        let tip2_x = tip_x - tip_perp_cos;
        let tip2_y = tip_y - tip_perp_sin;

        // Convert to normalized coordinates
        let base1_nx = base1_x / screen_w * 2.0 - 1.0;
        let base1_ny = 1.0 - base1_y / screen_h * 2.0;
        let base2_nx = base2_x / screen_w * 2.0 - 1.0;
        let base2_ny = 1.0 - base2_y / screen_h * 2.0;
        let tip1_nx = tip1_x / screen_w * 2.0 - 1.0;
        let tip1_ny = 1.0 - tip1_y / screen_h * 2.0;
        let tip2_nx = tip2_x / screen_w * 2.0 - 1.0;
        let tip2_ny = 1.0 - tip2_y / screen_h * 2.0;

        vec![
            // First triangle: base1 -> base2 -> tip1
            base1_nx, base1_ny, color.0, color.1, color.2,
            base2_nx, base2_ny, color.0, color.1, color.2,
            tip1_nx, tip1_ny, color.0, color.1, color.2,
            // Second triangle: base2 -> tip2 -> tip1
            base2_nx, base2_ny, color.0, color.1, color.2,
            tip2_nx, tip2_ny, color.0, color.1, color.2,
            tip1_nx, tip1_ny, color.0, color.1, color.2,
        ]
    }
}

/// Needle shape drawing a single radial tick mark: a `width`-wide rectangle spanning from
/// `length - mark_length` to `length` -- i.e. `length` is the mark's outer/tip radius, and
/// it extends inward from there, rather than the default blade's center-to-tip span. Useful
/// for needles that mark a point on a fixed ring rather than a hand reaching out from the
/// hub, e.g. a heading-accuracy mark placed just inside a compass's major-mark ring.
pub struct MarkNeedleShape {
    width: f32,
    mark_length: f32,
}

impl MarkNeedleShape {
    pub fn new(width: f32, mark_length: f32) -> Self {
        Self { width, mark_length }
    }
}

impl NeedleShape for MarkNeedleShape {
    fn vertices(&self, center_x: f32, center_y: f32, length: f32, angle: f32,
                color: (f32, f32, f32), screen_w: f32, screen_h: f32) -> Vec<f32> {
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        let outer_r = length;
        let inner_r = length - self.mark_length;

        let outer_x = center_x + cos_a * outer_r;
        let outer_y = center_y + sin_a * outer_r;
        let inner_x = center_x + cos_a * inner_r;
        let inner_y = center_y + sin_a * inner_r;

        let perp_x = -sin_a * self.width * 0.5;
        let perp_y = cos_a * self.width * 0.5;

        let to_ndc = |x: f32, y: f32| (x / screen_w * 2.0 - 1.0, 1.0 - y / screen_h * 2.0);
        let (i1x, i1y) = to_ndc(inner_x + perp_x, inner_y + perp_y);
        let (i2x, i2y) = to_ndc(inner_x - perp_x, inner_y - perp_y);
        let (o1x, o1y) = to_ndc(outer_x + perp_x, outer_y + perp_y);
        let (o2x, o2y) = to_ndc(outer_x - perp_x, outer_y - perp_y);

        vec![
            i1x, i1y, color.0, color.1, color.2,
            i2x, i2y, color.0, color.1, color.2,
            o1x, o1y, color.0, color.1, color.2,
            i2x, i2y, color.0, color.1, color.2,
            o2x, o2y, color.0, color.1, color.2,
            o1x, o1y, color.0, color.1, color.2,
        ]
    }
}
