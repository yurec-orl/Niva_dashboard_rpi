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
    let needle_length = ui_style.get_float(GAUGE_NEEDLE_LENGTH, 0.8);
    let needle_base_width = ui_style.get_float(GAUGE_NEEDLE_WIDTH, 8.0);
    let needle_tip_width = ui_style.get_float(GAUGE_NEEDLE_TIP_WIDTH, 1.0);

    // Border arc parameters
    let arc_width = ui_style.get_float(GAUGE_INACTIVE_ZONE_WIDTH, 4.0);

    // Label styling from UI configuration
    let gauge_labels_font = ui_style.get_string(GAUGE_LABEL_FONT, DEFAULT_GLOBAL_FONT_PATH);
    let gauge_labels_font_size = ui_style.get_integer(GAUGE_LABEL_FONT_SIZE, 10) as u32;
    let gauge_labels_offset = ui_style.get_float(GAUGE_LABEL_OFFSET, -35.0);

    // Style parameters from UI configuration
    let gauge_minor_mark_length = ui_style.get_float(GAUGE_MINOR_MARK_LENGTH, 6.0);
    let gauge_minor_mark_thickness = ui_style.get_float(GAUGE_MINOR_MARK_WIDTH, 2.0);
    let gauge_major_mark_length = ui_style.get_float(GAUGE_MAJOR_MARK_LENGTH, 12.0);
    let gauge_major_mark_thickness = ui_style.get_float(GAUGE_MAJOR_MARK_WIDTH, 4.0);

    let unit_offset_h = ui_style.get_float(GAUGE_UNIT_OFFSET_H, 0.0);
    let unit_offset_v = ui_style.get_float(GAUGE_UNIT_OFFSET_V, 20.0);

    let voltage_gauge = NeedleIndicator::new(
        start_angle,
        end_angle,
        needle_length,
        needle_base_width,
        needle_tip_width,
        GAUGE_NEEDLE_COLOR,
    ).with_decorators(vec![
        // Active arc (white) covering the valid range
        Box::new(ArcDecorator::new(
            radius,
            arc_width,
            GAUGE_BORDER_COLOR,
            start_angle,
            end_angle,
        )),
        // Inactive arc (dark grey) for the remaining circle
        Box::new(ArcDecorator::new(
            radius,
            arc_width, // Arc thickness
            GAUGE_INACTIVE_ZONE_COLOR,
            end_angle,
            start_angle + 2.0 * PI, // Complete the circle
        )),
        // Critical voltage zone arc (red) at high end (15-16V): last 1V = 45° before end_angle
        Box::new(ArcDecorator::new(
            radius - gauge_major_mark_length / 4.0,
            gauge_major_mark_length / 2.0,
            GAUGE_CRITICAL_ZONE_COLOR,
            end_angle - 45.0f32.to_radians(), // 15-16V range (1V = 45° out of 270°/6V)
            end_angle,
        )),
        // Critical voltage zone arc (red) at low end (10-12V): first 2V = 90° from start_angle
        Box::new(ArcDecorator::new(
            radius - gauge_major_mark_length / 4.0,
            gauge_major_mark_length / 2.0,
            GAUGE_CRITICAL_ZONE_COLOR,
            start_angle,
            start_angle + 90.0f32.to_radians(), // 10-12V range (2V = 90°)
        )),
        // Warning zone arc (orange) at lower-mid range (12-13.5V): 1.5V = 67.5° from +90°
        Box::new(ArcDecorator::new(
            radius - gauge_major_mark_length / 4.0,
            gauge_major_mark_length / 2.0,
            GAUGE_WARNING_ZONE_COLOR,
            start_angle + 90.0f32.to_radians(),   // 12V
            start_angle + 157.5f32.to_radians(),  // 13.5V (1.5V × 45°)
        )),
        // Normal zone arc (green) at optimal range (13.5-14.5V): 1V = 45° from +157.5°
        Box::new(ArcDecorator::new(
            radius - gauge_major_mark_length / 2.0,
            gauge_major_mark_length,
            GAUGE_NORMAL_ZONE_COLOR,
            start_angle + 157.5f32.to_radians(),  // 13.5V
            start_angle + 202.5f32.to_radians(),  // 14.5V (1V × 45°)
        )),
        // Warning zone arc (orange) at upper-mid range (14.5-15V): 0.5V = 22.5° from +202.5°
        Box::new(ArcDecorator::new(
            radius - gauge_major_mark_length / 4.0,
            gauge_major_mark_length / 2.0,
            GAUGE_WARNING_ZONE_COLOR,
            start_angle + 202.5f32.to_radians(),  // 14.5V
            start_angle + 225.0f32.to_radians(),  // 15V (= end_angle - 45°)
        )),
        // Fine marks for voltage readings (10-16V, every 0.5V = 13 marks)
        Box::new(NeedleGaugeMarksDecorator::new(
            13,
            gauge_minor_mark_length,
            gauge_minor_mark_thickness,
            GAUGE_MINOR_MARK_COLOR,
            radius,
            start_angle,
            end_angle,
        )),
        // Major marks for main voltage levels (10-16V, every 1V = 7 marks)
        Box::new(NeedleGaugeMarksDecorator::new(
            7,
            gauge_major_mark_length,
            gauge_major_mark_thickness,
            GAUGE_MAJOR_MARK_COLOR,
            radius,
            start_angle,
            end_angle,
        )),
        Box::new(LabelDecorator::new( // Voltage unit label at bottom
            "В".to_string(),
            ui_style.get_string(GAUGE_UNIT_FONT, DEFAULT_GLOBAL_FONT_PATH),
            ui_style.get_integer(GAUGE_UNIT_FONT_SIZE, 14),
            GAUGE_UNIT_COLOR,
            DecoratorAlignmentH::Center,
            DecoratorAlignmentV::Center,
        ).with_offset(unit_offset_h, unit_offset_v)),
        // Voltage level labels (10-16V)
        Box::new(NeedleGaugeMarkLabelsDecorator::new(
            vec!["10".into(), "11".into(), "12".into(), "13".into(), "14".into(), "15".into(), "16".into()],
            gauge_labels_font,
            gauge_labels_font_size,
            GAUGE_LABEL_COLOR,
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
