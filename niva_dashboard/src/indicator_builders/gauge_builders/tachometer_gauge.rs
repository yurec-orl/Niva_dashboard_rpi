use crate::indicators::{Indicator, IndicatorBounds};
use crate::indicators::needle_indicator::{NeedleIndicator, NeedleGaugeMarksDecorator, NeedleGaugeMarkLabelsDecorator};
use crate::indicators::decorator::{LabelDecorator, ArcDecorator, DecoratorAlignmentH, DecoratorAlignmentV};
use crate::graphics::ui_style::*;
use std::f32::consts::PI;

/// Build a tachometer gauge with customizable center point, radius and styling
///
/// # Parameters
/// - `center_x`: X coordinate of the gauge center
/// - `center_y`: Y coordinate of the gauge center
/// - `radius`: Radius of the gauge
/// - `ui_style`: UI styling configuration
///
/// # Returns
/// A boxed tachometer gauge indicator ready for rendering
pub fn build_tachometer_gauge(
    center_x: f32,
    center_y: f32,
    radius: f32,
    ui_style: &UIStyle,
) -> (Box<dyn Indicator>, IndicatorBounds) {
    // Tachometer configuration -- same overall arc span as the speedometer
    let start_angle = -225.0f32.to_radians(); // Start at 7 o'clock position
    let end_angle = 45.0f32.to_radians();     // End at 1 o'clock position
    let needle_length = ui_style.get_float(StyleKey::GaugeNeedleLength);
    let needle_base_width = ui_style.get_float(StyleKey::GaugeNeedleWidth);
    let needle_tip_width = ui_style.get_float(StyleKey::GaugeNeedleTipWidth);

    // Border arc parameters
    let arc_width = ui_style.get_float(StyleKey::GaugeInactiveZoneWidth);

    // Label styling from UI configuration
    let gauge_labels_font = ui_style.get_string(StyleKey::GaugeLabelFont);
    let gauge_labels_font_size = ui_style.get_integer(StyleKey::GaugeLabelFontSize);
    let gauge_labels_offset = ui_style.get_float(StyleKey::GaugeLabelOffset);

    // Mark styling
    let gauge_minor_mark_length = ui_style.get_float(StyleKey::GaugeMinorMarkLength);
    let gauge_minor_mark_thickness = ui_style.get_float(StyleKey::GaugeMinorMarkWidth);
    let gauge_major_mark_length = ui_style.get_float(StyleKey::GaugeMajorMarkLength);
    let gauge_major_mark_thickness = ui_style.get_float(StyleKey::GaugeMajorMarkWidth);

    let unit_offset_h = ui_style.get_float(StyleKey::GaugeUnitOffsetH);
    let unit_offset_v = ui_style.get_float(StyleKey::GaugeUnitOffsetV);

    // Needle scale covers the full 0-8000 rpm so idle/off parks the needle at the arc's
    // physical start. Mark angles are derived from the same value->angle mapping the
    // needle itself uses, so they stay aligned with where the needle actually points.
    let scale_min = 0.0f32;
    let scale_max = 8000.0f32;
    let angle_range = end_angle - start_angle;
    let value_to_angle = |v: f32| start_angle + (v - scale_min) / (scale_max - scale_min) * angle_range;

    let major_count = 9u32; // 0..=8000 rpm in steps of 1000 ("0".."80")
    let marks_start_angle = value_to_angle(0.0);
    let marks_end_angle = value_to_angle(8000.0);
    let major_step = (marks_end_angle - marks_start_angle) / (major_count - 1) as f32;
    let minor_count = major_count - 1; // one minor mark between each pair of major marks
    let minors_start_angle = marks_start_angle + major_step / 2.0;
    let minors_end_angle = marks_end_angle - major_step / 2.0;

    let tachometer = NeedleIndicator::new(
        start_angle,
        end_angle,
        needle_length,
        needle_base_width,
        needle_tip_width,
        StyleKey::GaugeNeedleColor,
    ).with_scale(scale_min, scale_max)
    .with_decorators(vec![
        // Minor marks, unlabeled -- one between each pair of major marks
        Box::new(NeedleGaugeMarksDecorator::new(
            minor_count,
            gauge_minor_mark_length,
            gauge_minor_mark_thickness,
            StyleKey::GaugeMinorMarkColor,
            radius,
            minors_start_angle,
            minors_end_angle,
        )),
        // Major marks, labeled 0..80 (rpm/100, i.e. 0..8000 rpm)
        Box::new(NeedleGaugeMarksDecorator::new(
            major_count,
            gauge_major_mark_length,
            gauge_major_mark_thickness,
            StyleKey::GaugeMajorMarkColor,
            radius,
            marks_start_angle,
            marks_end_angle,
        )),
        // Active arc (white) covering the full needle sweep
        Box::new(ArcDecorator::new(
            radius,
            arc_width,
            StyleKey::GaugeBorderColor,
            start_angle,
            end_angle,
        )),
        // Inactive arc (dark grey) for the remaining circle
        Box::new(ArcDecorator::new(
            radius,
            arc_width, // Arc thickness
            StyleKey::GaugeInactiveZoneColor,
            end_angle,
            start_angle + 2.0 * PI, // Complete the circle
        )),
        Box::new(LabelDecorator::new( // Unit label at bottom
            "x100".to_string(),
            ui_style.get_string(StyleKey::GaugeUnitFont),
            ui_style.get_integer(StyleKey::GaugeUnitFontSize),
            StyleKey::GaugeUnitColor,
            DecoratorAlignmentH::Center,
            DecoratorAlignmentV::Center,
        ).with_offset(unit_offset_h, unit_offset_v)), // slight offset to avoid overlap
        Box::new(NeedleGaugeMarkLabelsDecorator::new(
            (0..=8).map(|v| (v * 10).to_string()).collect(), // 0, 10, 20, ..., 80
            gauge_labels_font,
            gauge_labels_font_size,
            StyleKey::GaugeLabelColor,
            radius + gauge_labels_offset, // Negative offset moves labels inside the gauge
            marks_start_angle,
            marks_end_angle,
        )),
    ]);

    let bounds = IndicatorBounds::new(
        center_x - radius,
        center_y - radius,
        radius * 2.0,
        radius * 2.0,
    );

    (Box::new(tachometer), bounds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_tachometer_gauge_bounds_and_type() {
        let ui_style = UIStyle::from_file(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui_style.json"))
            .expect("repo ui_style.json should load and validate");
        let (indicator, bounds) = build_tachometer_gauge(400.0, 240.0, 180.0, &ui_style);

        assert_eq!(bounds.x, 220.0);
        assert_eq!(bounds.y, 60.0);
        assert_eq!(bounds.width, 360.0);
        assert_eq!(bounds.height, 360.0);
        assert_eq!(indicator.indicator_type(), "NeedleIndicator");
    }
}
