use crate::indicators::{Indicator, IndicatorBounds};
use crate::indicators::needle_indicator::{NeedleIndicator, NeedleGaugeMarksDecorator, NeedleGaugeMarkLabelsDecorator};
use crate::indicators::decorator::{ArcDecorator};
use crate::graphics::ui_style::*;
use std::f32::consts::PI;

/// Build a fuel level gauge with customizable center point, radius and styling
/// 
/// # Parameters
/// - `center_x`: X coordinate of the gauge center
/// - `center_y`: Y coordinate of the gauge center  
/// - `radius`: Radius of the gauge
/// - `ui_style`: UI styling configuration
///
/// # Returns
/// A boxed fuel level gauge indicator ready for rendering
pub fn build_fuel_level_gauge(
    center_x: f32,
    center_y: f32,
    radius: f32,
    ui_style: &UIStyle,
) -> (Box<dyn Indicator>, IndicatorBounds) {
    // Fuel level gauge configuration
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

    // Mark lengths and thicknesses
    let gauge_minor_mark_length = ui_style.get_float(StyleKey::GaugeMinorMarkLength);
    let gauge_minor_mark_thickness = ui_style.get_float(StyleKey::GaugeMinorMarkWidth);
    let gauge_major_mark_length = ui_style.get_float(StyleKey::GaugeMajorMarkLength);
    let gauge_major_mark_thickness = ui_style.get_float(StyleKey::GaugeMajorMarkWidth);

    let _unit_offset_h = ui_style.get_float(StyleKey::GaugeUnitOffsetH);
    let _unit_offset_v = ui_style.get_float(StyleKey::GaugeUnitOffsetV);

    let fuel_gauge = NeedleIndicator::new(
        start_angle,
        end_angle,
        needle_length,
        needle_base_width,
        needle_tip_width,
        StyleKey::GaugeNeedleColor,
    ).with_scale(0.0, 100.0) // Matches the empty..full (0-100%) marks/labels below
    .with_decorators(vec![
        // Active arc (white) covering the valid range
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
        // Warning zone arc (yellow) at low end
        Box::new(ArcDecorator::new(
            radius - gauge_major_mark_length / 2.0,
            gauge_major_mark_length,    // Thick arc section to mark low fuel
            StyleKey::GaugeWarningZoneColor,
            start_angle,
            start_angle + 45.0f32.to_radians(),
        )),
        // Fine marks for fuel level readings
        Box::new(NeedleGaugeMarksDecorator::new(
            7, // 7 marks
            gauge_minor_mark_length,
            gauge_minor_mark_thickness,
            StyleKey::GaugeMinorMarkColor,
            radius,
            start_angle,
            end_angle,
        )),
        // Major marks for main fuel levels (Empty, Half, Full)
        Box::new(NeedleGaugeMarksDecorator::new(
            3, // 3 major marks (Empty, Half, Full)
            gauge_major_mark_length,
            gauge_major_mark_thickness,
            StyleKey::GaugeMajorMarkColor,
            radius,
            start_angle,
            end_angle,
        )),
        // Fuel level labels
        Box::new(NeedleGaugeMarkLabelsDecorator::new(
            vec!["0".into(), "1/2".into(), "4/4".into()], // Empty, Half, Full labels
            gauge_labels_font,
            gauge_labels_font_size,
            StyleKey::GaugeLabelColor,
            radius + gauge_labels_offset, // Negative offset moves labels inside the gauge
            start_angle,
            end_angle,
        )),
    ]);

    let bounds = IndicatorBounds::new(
        center_x - radius,
        center_y - radius,
        radius * 2.0,
        radius * 2.0,
    );

    (Box::new(fuel_gauge), bounds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_fuel_level_gauge_bounds_and_type() {
        let ui_style = UIStyle::from_file(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui_style.json"))
            .expect("repo ui_style.json should load and validate");
        let (indicator, bounds) = build_fuel_level_gauge(400.0, 240.0, 180.0, &ui_style);

        assert_eq!(bounds.x, 220.0);
        assert_eq!(bounds.y, 60.0);
        assert_eq!(bounds.width, 360.0);
        assert_eq!(bounds.height, 360.0);
        assert_eq!(indicator.indicator_type(), "NeedleIndicator");
    }
}
