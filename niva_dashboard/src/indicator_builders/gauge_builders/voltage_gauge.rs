use crate::indicators::{Indicator, IndicatorBounds};
use crate::indicators::needle_indicator::{NeedleIndicator, NeedleGaugeMarksDecorator, NeedleGaugeMarkLabelsDecorator};
use crate::indicators::decorator::{LabelDecorator, ArcDecorator, DecoratorAlignmentH, DecoratorAlignmentV};
use crate::graphics::ui_style::*;
use std::f32::consts::PI;

/// Build a voltage gauge with customizable center point, radius and styling
/// 
/// # Parameters
/// - `center_x`: X coordinate of the gauge center
/// - `center_y`: Y coordinate of the gauge center  
/// - `radius`: Radius of the gauge
/// - `ui_style`: UI styling configuration
///
/// # Returns
/// A boxed voltage gauge indicator ready for rendering
pub fn build_voltage_gauge(
    center_x: f32,
    center_y: f32,
    radius: f32,
    ui_style: &UIStyle,
) -> (Box<dyn Indicator>, IndicatorBounds) {
    // Voltage gauge configuration
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

    // Style parameters from UI configuration
    let gauge_minor_mark_length = ui_style.get_float(StyleKey::GaugeMinorMarkLength);
    let gauge_minor_mark_thickness = ui_style.get_float(StyleKey::GaugeMinorMarkWidth);
    let gauge_major_mark_length = ui_style.get_float(StyleKey::GaugeMajorMarkLength);
    let gauge_major_mark_thickness = ui_style.get_float(StyleKey::GaugeMajorMarkWidth);

    let unit_offset_h = ui_style.get_float(StyleKey::GaugeUnitOffsetH);
    let unit_offset_v = ui_style.get_float(StyleKey::GaugeUnitOffsetV);

    let voltage_gauge = NeedleIndicator::new(
        start_angle,
        end_angle,
        needle_length,
        needle_base_width,
        needle_tip_width,
        StyleKey::GaugeNeedleColor,
    ).with_scale(10.0, 16.0) // Matches the 10-16 V marks/labels below
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
        // Critical voltage zone arc (red) at high end (15-16V): last 1V = 45° before end_angle
        Box::new(ArcDecorator::new(
            radius - gauge_major_mark_length / 4.0,
            gauge_major_mark_length / 2.0,
            StyleKey::GaugeCriticalZoneColor,
            end_angle - 45.0f32.to_radians(), // 15-16V range (1V = 45° out of 270°/6V)
            end_angle,
        )),
        // Critical voltage zone arc (red) at low end (10-12V): first 2V = 90° from start_angle
        Box::new(ArcDecorator::new(
            radius - gauge_major_mark_length / 4.0,
            gauge_major_mark_length / 2.0,
            StyleKey::GaugeCriticalZoneColor,
            start_angle,
            start_angle + 90.0f32.to_radians(), // 10-12V range (2V = 90°)
        )),
        // Warning zone arc (orange) at lower-mid range (12-13.5V): 1.5V = 67.5° from +90°
        Box::new(ArcDecorator::new(
            radius - gauge_major_mark_length / 4.0,
            gauge_major_mark_length / 2.0,
            StyleKey::GaugeWarningZoneColor,
            start_angle + 90.0f32.to_radians(),   // 12V
            start_angle + 157.5f32.to_radians(),  // 13.5V (1.5V × 45°)
        )),
        // Normal zone arc (green) at optimal range (13.5-14.5V): 1V = 45° from +157.5°
        Box::new(ArcDecorator::new(
            radius - gauge_major_mark_length / 2.0,
            gauge_major_mark_length,
            StyleKey::GaugeNormalZoneColor,
            start_angle + 157.5f32.to_radians(),  // 13.5V
            start_angle + 202.5f32.to_radians(),  // 14.5V (1V × 45°)
        )),
        // Warning zone arc (orange) at upper-mid range (14.5-15V): 0.5V = 22.5° from +202.5°
        Box::new(ArcDecorator::new(
            radius - gauge_major_mark_length / 4.0,
            gauge_major_mark_length / 2.0,
            StyleKey::GaugeWarningZoneColor,
            start_angle + 202.5f32.to_radians(),  // 14.5V
            start_angle + 225.0f32.to_radians(),  // 15V (= end_angle - 45°)
        )),
        // Fine marks for voltage readings (10-16V, every 0.5V = 13 marks)
        Box::new(NeedleGaugeMarksDecorator::new(
            13,
            gauge_minor_mark_length,
            gauge_minor_mark_thickness,
            StyleKey::GaugeMinorMarkColor,
            radius,
            start_angle,
            end_angle,
        )),
        // Major marks for main voltage levels (10-16V, every 1V = 7 marks)
        Box::new(NeedleGaugeMarksDecorator::new(
            7,
            gauge_major_mark_length,
            gauge_major_mark_thickness,
            StyleKey::GaugeMajorMarkColor,
            radius,
            start_angle,
            end_angle,
        )),
        Box::new(LabelDecorator::new( // Voltage unit label at bottom
            "В".to_string(),
            ui_style.get_string(StyleKey::GaugeUnitFont),
            ui_style.get_integer(StyleKey::GaugeUnitFontSize),
            StyleKey::GaugeUnitColor,
            DecoratorAlignmentH::Center,
            DecoratorAlignmentV::Center,
        ).with_offset(unit_offset_h, unit_offset_v)),
        // Voltage level labels (10-16V)
        Box::new(NeedleGaugeMarkLabelsDecorator::new(
            vec!["10".into(), "11".into(), "12".into(), "13".into(), "14".into(), "15".into(), "16".into()],
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

    (Box::new(voltage_gauge), bounds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_voltage_gauge_bounds_and_type() {
        let ui_style = UIStyle::from_file(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui_style.json"))
            .expect("repo ui_style.json should load and validate");
        let (indicator, bounds) = build_voltage_gauge(400.0, 240.0, 150.0, &ui_style);

        assert_eq!(bounds.x, 250.0);
        assert_eq!(bounds.y, 90.0);
        assert_eq!(bounds.width, 300.0);
        assert_eq!(bounds.height, 300.0);
        assert_eq!(indicator.indicator_type(), "NeedleIndicator");
    }
}
