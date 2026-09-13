use crate::indicators::{Indicator, IndicatorBounds};
use crate::indicators::needle_indicator::{NeedleIndicator, NeedleGaugeMarksDecorator, NeedleGaugeMarkLabelsDecorator};
use crate::indicators::decorator::{LabelDecorator, ArcDecorator, DecoratorAlignmentH, DecoratorAlignmentV};
use crate::graphics::ui_style::*;
use std::f32::consts::PI;

/// Build an oil pressure gauge with customizable center point, radius and styling
/// 
/// # Parameters
/// - `center_x`: X coordinate of the gauge center
/// - `center_y`: Y coordinate of the gauge center  
/// - `radius`: Radius of the gauge
/// - `ui_style`: UI styling configuration
///
/// # Returns
/// A boxed oil pressure gauge indicator ready for rendering
pub fn build_oil_pressure_gauge(
    center_x: f32,
    center_y: f32,
    radius: f32,
    ui_style: &UIStyle,
) -> (Box<dyn Indicator>, IndicatorBounds) {
    // Oil pressure gauge configuration
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

    let oil_pressure_gauge = NeedleIndicator::new(
        start_angle,
        end_angle,
        needle_length,
        needle_base_width,
        needle_tip_width,
        StyleKey::GaugeNeedleColor,
    ).with_scale(0.0, 8.0) // Matches the 0-8 kgf/cm² marks/labels below
    .with_decorators(vec![
        // Fine marks for oil pressure readings (0-8 kgf/cm²)
        Box::new(NeedleGaugeMarksDecorator::new(
            9, // 9 marks for oil pressure range - from 0 to 8
            gauge_minor_mark_length,
            gauge_minor_mark_thickness,
            StyleKey::GaugeMinorMarkColor,
            radius,
            start_angle,
            end_angle,
        )),
        // Major marks for main oil pressure levels
        Box::new(NeedleGaugeMarksDecorator::new(
            3, // 3 major marks (Low 0, Normal 4, High 8)
            gauge_major_mark_length,
            gauge_major_mark_thickness,
            StyleKey::GaugeMajorMarkColor,
            radius,
            start_angle,
            end_angle,
        )),
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
        // Critical pressure zone arc (red) at low end (0-0.5)
        Box::new(ArcDecorator::new(
            radius - gauge_major_mark_length / 2.0,
            gauge_major_mark_length,    // Thick arc section to mark critical temp
            StyleKey::GaugeCriticalZoneColor,
            start_angle,
            start_angle + 16.875f32.to_radians(), // 0-0.5 kgf/cm² range
        )),
        Box::new(LabelDecorator::new( // Oil pressure unit label at bottom
            "кгс/см²".to_string(),
            ui_style.get_string(StyleKey::GaugeUnitFont),
            ui_style.get_integer(StyleKey::GaugeUnitFontSize),
            StyleKey::GaugeUnitColor,
            DecoratorAlignmentH::Center,
            DecoratorAlignmentV::Center,
        ).with_offset(unit_offset_h, unit_offset_v)),
        // Oil pressure level labels
        Box::new(NeedleGaugeMarkLabelsDecorator::new(
            vec!["0".into(), "4".into(), "8".into()], // Oil pressure labels in kgf/cm²
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

    (Box::new(oil_pressure_gauge), bounds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_oil_pressure_gauge_bounds_and_type() {
        let ui_style = UIStyle::from_file(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui_style.json"))
            .expect("repo ui_style.json should load and validate");
        let (indicator, bounds) = build_oil_pressure_gauge(400.0, 240.0, 150.0, &ui_style);

        assert_eq!(bounds.x, 250.0);
        assert_eq!(bounds.y, 90.0);
        assert_eq!(bounds.width, 300.0);
        assert_eq!(bounds.height, 300.0);
        assert_eq!(indicator.indicator_type(), "NeedleIndicator");
    }
}
